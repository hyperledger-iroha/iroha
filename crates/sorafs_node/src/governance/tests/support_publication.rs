// Shared publication recovery fixtures and runtime DAG test support.

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn recovery_quarantine_path(root: &Path) -> PathBuf {
    root.join(GOVERNANCE_PUBLICATION_RECOVERY_QUARANTINE_DIR)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn clear_recovery_quarantine_offline(root: &Path) {
    let quarantine = recovery_quarantine_path(root);
    assert!(
        quarantine.is_dir(),
        "offline cleanup requires a preserved recovery quarantine"
    );
    fs::remove_dir_all(quarantine).expect("clear recovery quarantine while publisher is stopped");
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn finish_recovery_after_offline_quarantine_cleanup(root: &Path) -> FilesystemGovernancePublisher {
    for _ in 0..3 {
        match FilesystemGovernancePublisher::try_new(root.to_path_buf()) {
            Ok(publisher) => return publisher,
            Err(error)
                if error
                    .to_string()
                    .contains(GOVERNANCE_PUBLICATION_RECOVERY_QUARANTINE_DIR) =>
            {
                clear_recovery_quarantine_offline(root);
            }
            Err(error) => panic!("restart after offline quarantine cleanup failed: {error}"),
        }
    }
    panic!("recovery did not converge after bounded offline quarantine cleanup")
}

fn committed_publication_artifact_paths(
    root: &Path,
    state: &JsonMap,
) -> Vec<(&'static str, PathBuf)> {
    let entry = state
        .get("publish_index")
        .and_then(|index| index.get("entries"))
        .and_then(JsonValue::as_array)
        .and_then(|entries| entries.first())
        .and_then(JsonValue::as_object)
        .expect("committed publish entry");
    let segment = state
        .get("car_queue")
        .and_then(|queue| queue.get("segments"))
        .and_then(JsonValue::as_array)
        .and_then(|segments| segments.first())
        .and_then(JsonValue::as_object)
        .expect("committed CAR segment");
    let encoded = root.join(
        entry
            .get("encoded_path")
            .and_then(JsonValue::as_str)
            .expect("committed encoded path"),
    );
    let json = root.join(
        entry
            .get("json_path")
            .and_then(JsonValue::as_str)
            .expect("committed JSON path"),
    );
    let car = root.join(
        segment
            .get("car_path")
            .and_then(JsonValue::as_str)
            .expect("committed CAR path"),
    );
    let plan = root.join(
        segment
            .get("plan_path")
            .and_then(JsonValue::as_str)
            .expect("committed CAR plan path"),
    );
    let manifest = root.join(
        segment
            .get("manifest_path")
            .and_then(JsonValue::as_str)
            .expect("committed CAR manifest path"),
    );
    vec![
        ("encoded source", encoded.clone()),
        ("encoded source sidecar", digest_sidecar_path_for(&encoded)),
        ("JSON source", json.clone()),
        ("JSON source sidecar", digest_sidecar_path_for(&json)),
        ("CAR archive", car.clone()),
        ("CAR archive sidecar", digest_sidecar_path_for(&car)),
        ("CAR plan", plan.clone()),
        ("CAR plan sidecar", digest_sidecar_path_for(&plan)),
        ("CAR manifest", manifest.clone()),
        ("CAR manifest sidecar", digest_sidecar_path_for(&manifest)),
    ]
}

#[test]
fn governance_car_segment_sources_require_recorded_length_digest_and_file_caps() {
    let temp = tempdir().expect("tempdir");
    let encoded = b"canonical-payload";
    let entry = write_car_segment_source_fixture(temp.path(), encoded);
    let root_guard =
        GovernanceFilesystemRootGuard::capture_writer(temp.path()).expect("retain CAR source root");

    let (files, records) = governance_car_segment_files(temp.path(), &root_guard, &entry)
        .expect("read canonical CAR sources");
    assert_eq!(files.len(), 4);
    assert_eq!(records.len(), 4);

    let mut wrong_length = entry.clone();
    wrong_length.encoded_len += 1;
    let error = governance_car_segment_files(temp.path(), &root_guard, &wrong_length)
        .expect_err("shorter encoded source must not satisfy its recorded length");
    assert!(error.to_string().contains("encoded source"));

    let encoded_path = temp.path().join(&entry.encoded_path);
    let substituted_encoded = b"tampered!-payload";
    assert_eq!(substituted_encoded.len(), encoded.len());
    fs::write(&encoded_path, substituted_encoded).expect("substitute encoded source");
    let mut substituted_sidecar = blake3::hash(substituted_encoded).to_hex().to_string();
    substituted_sidecar.push('\n');
    fs::write(digest_sidecar_path_for(&encoded_path), substituted_sidecar)
        .expect("substitute matching encoded sidecar");
    let error = governance_car_segment_files(temp.path(), &root_guard, &entry)
        .expect_err("same-length encoded plus sidecar substitution must fail closed");
    assert!(error.to_string().contains("encoded source"));

    let entry = write_car_segment_source_fixture(temp.path(), encoded);
    let json_path = temp.path().join(&entry.json_path);
    let substituted_json = br#"{"status":"owned"}"#;
    assert_eq!(substituted_json.len(), entry.json_len);
    fs::write(&json_path, substituted_json).expect("substitute JSON source");
    let mut substituted_sidecar = blake3::hash(substituted_json).to_hex().to_string();
    substituted_sidecar.push('\n');
    fs::write(digest_sidecar_path_for(&json_path), substituted_sidecar)
        .expect("substitute matching JSON sidecar");
    let error = governance_car_segment_files(temp.path(), &root_guard, &entry)
        .expect_err("same-length JSON plus sidecar substitution must fail closed");
    assert!(error.to_string().contains("JSON source"));

    let entry = write_car_segment_source_fixture(temp.path(), encoded);
    fs::write(
        digest_sidecar_path_for(&temp.path().join(&entry.encoded_path)),
        format!("{}\n", "0".repeat(64)),
    )
    .expect("substitute encoded digest sidecar");
    let error = governance_car_segment_files(temp.path(), &root_guard, &entry)
        .expect_err("a mismatched retained digest sidecar must fail closed");
    assert!(error.to_string().contains("digest sidecar"));

    let entry = write_car_segment_source_fixture(temp.path(), encoded);
    fs::write(
        digest_sidecar_path_for(&temp.path().join(&entry.encoded_path)),
        vec![b'0'; GOVERNANCE_DIGEST_SIDECAR_BYTES + 1],
    )
    .expect("write oversized digest sidecar");
    let error = governance_car_segment_files(temp.path(), &root_guard, &entry)
        .expect_err("oversized digest sidecar must fail closed");
    assert!(
        error
            .to_string()
            .contains(&format!("exceeds {GOVERNANCE_DIGEST_SIDECAR_BYTES} bytes"))
    );

    let mut corrupted_index_entry = entry;
    corrupted_index_entry.encoded_len = GOVERNANCE_CAR_SOURCE_ENCODED_MAX_BYTES + 1;
    let error = governance_car_segment_files(temp.path(), &root_guard, &corrupted_index_entry)
        .expect_err("corrupted publish-index length must fail before its source read");
    assert!(error.to_string().contains("encoded publication length"));
}

#[test]
fn governance_car_source_limits_cover_each_file_and_the_checked_segment_total() {
    assert_eq!(
        validate_governance_car_source_lengths(
            GOVERNANCE_CAR_SOURCE_ENCODED_MAX_BYTES,
            GOVERNANCE_CAR_SOURCE_JSON_MAX_BYTES,
        )
        .expect("boundary lengths are valid"),
        GOVERNANCE_CAR_SOURCE_TOTAL_MAX_BYTES
    );
    for (encoded_len, json_len) in [
        (GOVERNANCE_CAR_SOURCE_ENCODED_MAX_BYTES + 1, 1),
        (1, GOVERNANCE_CAR_SOURCE_JSON_MAX_BYTES + 1),
        (0, 1),
        (1, 0),
    ] {
        validate_governance_car_source_lengths(encoded_len, json_len)
            .expect_err("outside-boundary governance CAR source lengths must fail");
    }
}

#[test]
fn governance_immutable_artifacts_are_exact_idempotent_and_non_overwritable() {
    let temp = tempdir().expect("tempdir");
    let root_guard = GovernanceFilesystemRootGuard::capture_writer(temp.path())
        .expect("retain publication root");
    let path = temp.path().join("sources").join("identity.to");
    let canonical = b"canonical-source";

    write_immutable_governance_artifact(
        &root_guard,
        &path,
        canonical,
        GOVERNANCE_CAR_SOURCE_ENCODED_MAX_BYTES,
    )
    .expect("create immutable source");
    write_immutable_governance_artifact(
        &root_guard,
        &path,
        canonical,
        GOVERNANCE_CAR_SOURCE_ENCODED_MAX_BYTES,
    )
    .expect("exact replay is idempotent");
    let error = write_immutable_governance_artifact(
        &root_guard,
        &path,
        b"substituted-source",
        GOVERNANCE_CAR_SOURCE_ENCODED_MAX_BYTES,
    )
    .expect_err("divergent replay must not replace immutable source bytes");
    assert!(error.to_string().contains("occupied by different bytes"));
    assert_eq!(fs::read(path).expect("read immutable source"), canonical);
}

#[test]
fn governance_publish_index_rejects_labels_above_the_fixed_cap() {
    let temp = tempdir().expect("tempdir");
    let fixture = write_car_segment_source_fixture(temp.path(), b"canonical-payload");
    let mut labels = JsonMap::new();
    for index in 0..=GOVERNANCE_PUBLICATION_LABEL_MAX_ENTRIES_V1 {
        labels.insert(format!("label_{index}"), JsonValue::from(index as u64));
    }

    let error = update_publish_index(
        temp.path(),
        empty_governance_publish_index(),
        &fixture.payload_kind,
        &temp.path().join(&fixture.encoded_path),
        &temp.path().join(&fixture.json_path),
        &fixture.encoded_blake3,
        fixture.encoded_len,
        &fixture.json_blake3,
        fixture.json_len,
        labels,
    )
    .expect_err("publish entries above the label cap must fail before CAR assembly");
    assert!(error.to_string().contains(&format!(
        "{GOVERNANCE_PUBLICATION_LABEL_MAX_ENTRIES_V1}-label hard cap"
    )));
}

#[test]
fn governance_publication_labels_enforce_canonical_scalar_and_byte_bounds() {
    let mut boundary = JsonMap::new();
    boundary.insert(
        "a".repeat(GOVERNANCE_PUBLICATION_LABEL_KEY_MAX_BYTES_V1),
        JsonValue::from("x".repeat(GOVERNANCE_PUBLICATION_LABEL_STRING_MAX_BYTES_V1)),
    );
    boundary.insert("boolean".into(), JsonValue::from(true));
    boundary.insert("null".into(), JsonValue::Null);
    boundary.insert("number".into(), JsonValue::from(7_u64));
    validate_governance_publication_labels(&boundary, "test publication")
        .expect("labels at the per-field boundaries remain valid");

    for key in [
        String::new(),
        "bad/key".to_owned(),
        "a".repeat(GOVERNANCE_PUBLICATION_LABEL_KEY_MAX_BYTES_V1 + 1),
    ] {
        let mut labels = JsonMap::new();
        labels.insert(key, JsonValue::from("value"));
        let error = validate_governance_publication_labels(&labels, "test publication")
            .expect_err("noncanonical label keys must fail closed");
        assert!(error.to_string().contains("noncanonical label key"));
    }

    for value in [
        JsonValue::Array(Vec::new()),
        JsonValue::Object(JsonMap::new()),
    ] {
        let mut labels = JsonMap::new();
        labels.insert("nested".into(), value);
        let error = validate_governance_publication_labels(&labels, "test publication")
            .expect_err("structured label values must fail closed");
        assert!(error.to_string().contains("must be a scalar"));
    }

    let mut oversized_string = JsonMap::new();
    oversized_string.insert(
        "value".into(),
        JsonValue::from("x".repeat(GOVERNANCE_PUBLICATION_LABEL_STRING_MAX_BYTES_V1 + 1)),
    );
    let error = validate_governance_publication_labels(&oversized_string, "test publication")
        .expect_err("oversized label strings must fail closed");
    assert!(error.to_string().contains("string bound"));

    let mut oversized_aggregate = JsonMap::new();
    for index in 0..16 {
        oversized_aggregate.insert(
            format!("label_{index:02}"),
            JsonValue::from("x".repeat(GOVERNANCE_PUBLICATION_LABEL_STRING_MAX_BYTES_V1)),
        );
    }
    let error = validate_governance_publication_labels(&oversized_aggregate, "test publication")
        .expect_err("oversized aggregate label metadata must fail closed");
    assert!(error.to_string().contains("aggregate bound"));
}

#[test]
fn governance_index_paths_enforce_fixed_byte_and_component_bounds() {
    let boundary = std::iter::repeat_n("a", GOVERNANCE_RELATIVE_PATH_MAX_COMPONENTS)
        .collect::<Vec<_>>()
        .join("/");
    assert_eq!(
        index_path_components(&boundary)
            .expect("path at the component-count boundary is valid")
            .len(),
        GOVERNANCE_RELATIVE_PATH_MAX_COMPONENTS
    );
    assert!(
        index_path_components(&"a".repeat(GOVERNANCE_RELATIVE_PATH_COMPONENT_MAX_BYTES)).is_ok(),
        "component at the byte boundary is valid"
    );

    let too_many_components = format!("{boundary}/a");
    assert!(index_path_components(&too_many_components).is_err());
    assert!(
        index_path_components(&"a".repeat(GOVERNANCE_RELATIVE_PATH_COMPONENT_MAX_BYTES + 1))
            .is_err()
    );
    assert!(index_path_components(&"a".repeat(GOVERNANCE_RELATIVE_PATH_MAX_BYTES + 1)).is_err());
}

#[test]
fn governance_publication_artifact_names_are_canonical_and_bounded() {
    let digest = "11".repeat(32);
    let oversized_kind = "a".repeat(129);
    for kind in [
        "",
        ".",
        "..",
        "../escape",
        "bad/kind",
        "Uppercase",
        oversized_kind.as_str(),
    ] {
        assert!(
            governance_source_pair_relative_paths(kind, 1, &digest, 1, &digest).is_err(),
            "publication kind `{kind}` must not become path authority"
        );
    }
    let (encoded, json) =
        governance_source_pair_relative_paths(&"a".repeat(128), 1, &digest, 1, &digest)
            .expect("publication kind at the byte boundary");
    assert!(encoded.ends_with("/payload.to"));
    assert!(json.ends_with("/payload.json"));

    let pair_id = "ab".repeat(32);
    assert!(is_canonical_governance_source_pair_directory(&pair_id));
    assert!(!is_canonical_governance_source_pair_directory(
        &pair_id.to_uppercase()
    ));
    for suffix in [
        ".car",
        ".car.blake3",
        ".plan.json",
        ".plan.json.blake3",
        ".json",
        ".json.blake3",
    ] {
        assert!(is_canonical_governance_car_artifact_name(&format!(
            "{:020}_{pair_id}{suffix}",
            7
        )));
    }
    assert!(!is_canonical_governance_car_artifact_name(&format!(
        "7_{pair_id}.car"
    )));
    assert!(!is_canonical_governance_car_artifact_name(&format!(
        "{:020}_{pair_id}.tmp",
        7
    )));
}

#[test]
fn governance_publication_state_commit_failure_preserves_immutable_orphans() {
    let temp = tempdir().expect("tempdir");
    let root_guard = GovernanceFilesystemRootGuard::capture_writer(temp.path())
        .expect("retain publication root");
    let _ = initialize_governance_publication_authority_if_pristine(temp.path(), &root_guard)
        .expect("initialize typed publication authority before immutable artifacts");
    let fixture = write_car_segment_source_fixture(temp.path(), b"canonical-payload");
    let encoded_path = temp.path().join(&fixture.encoded_path);
    let json_path = temp.path().join(&fixture.json_path);
    let (publish_index, entry) = update_publish_index(
        temp.path(),
        empty_governance_publish_index(),
        &fixture.payload_kind,
        &encoded_path,
        &json_path,
        &fixture.encoded_blake3,
        fixture.encoded_len,
        &fixture.json_blake3,
        fixture.json_len,
        JsonMap::new(),
    )
    .expect("prepare bounded publish index");
    let queue = assemble_governance_car_queue(
        temp.path(),
        &root_guard,
        empty_governance_car_queue(),
        &entry,
    )
    .expect("qualify CAR artifacts before commit");
    let car_path = resolve_index_path(
        temp.path(),
        queue
            .get("segments")
            .and_then(JsonValue::as_array)
            .and_then(|segments| segments.first())
            .and_then(|segment| segment.get("car_path"))
            .and_then(JsonValue::as_str)
            .expect("qualified CAR path"),
    )
    .expect("resolve qualified CAR path");
    let canonical_car = fs::read(&car_path).expect("read qualified CAR");
    let mut state = empty_governance_publication_state();
    state.insert(
        "publish_index".into(),
        JsonValue::Object(publish_index.clone()),
    );
    state.insert("car_queue".into(), JsonValue::Object(queue));

    commit_governance_publication_state_with(
        temp.path(),
        &root_guard,
        state,
        |_guard, _path, _bytes| Err(io::Error::other("injected commit failure")),
    )
    .expect_err("injected authoritative rename must fail");
    assert!(
        !temp.path().join(GOVERNANCE_PUBLICATION_STATE_FILE).exists(),
        "failed commit must not resurrect the retired flat-file authority"
    );
    assert_eq!(
        read_publication_state_fixture(temp.path())
            .get("generation")
            .and_then(JsonValue::as_u64),
        Some(0),
        "failed CAS must preserve the typed initial authority"
    );

    let retried_queue = assemble_governance_car_queue(
        temp.path(),
        &root_guard,
        empty_governance_car_queue(),
        &entry,
    )
    .expect("retry reuses exact immutable orphan artifacts");
    assert_eq!(fs::read(&car_path).expect("read reused CAR"), canonical_car);

    fs::write(&car_path, b"substituted orphan").expect("substitute unreachable orphan");
    let error = assemble_governance_car_queue(
        temp.path(),
        &root_guard,
        empty_governance_car_queue(),
        &entry,
    )
    .expect_err("retry must not replace a divergent immutable orphan");
    assert!(error.to_string().contains("occupied by different bytes"));

    fs::remove_file(&car_path).expect("remove divergent unreachable orphan");
    let repaired_queue = assemble_governance_car_queue(
        temp.path(),
        &root_guard,
        empty_governance_car_queue(),
        &entry,
    )
    .expect("retry recreates a missing immutable orphan from canonical sources");
    for field in [
        "segments",
        "by_encoded_blake3",
        "by_payload_kind",
        "by_car_archive_blake3",
        "segment_count",
        "assembled_count",
        "pending_count",
    ] {
        assert_eq!(
            retried_queue.get(field),
            repaired_queue.get(field),
            "canonical retry diverged at `{field}`"
        );
    }
    assert_eq!(
        fs::read(&car_path).expect("read recreated CAR"),
        canonical_car
    );
    let mut retry_state = empty_governance_publication_state();
    retry_state.insert("publish_index".into(), JsonValue::Object(publish_index));
    retry_state.insert("car_queue".into(), JsonValue::Object(repaired_queue));
    commit_governance_publication_state(temp.path(), &root_guard, retry_state)
        .expect("single authoritative retry commit");
    let committed = read_publication_state_fixture(temp.path());
    assert_eq!(
        committed.get("generation").and_then(JsonValue::as_u64),
        Some(1)
    );
    validate_governance_publication_state(
        committed.as_object().expect("committed publication state"),
    )
    .expect("committed cross-sections remain one-to-one");
}

#[test]
fn governance_publication_failed_successor_commit_preserves_exact_predecessor() {
    let temp = tempdir().expect("tempdir");
    let root_guard = GovernanceFilesystemRootGuard::capture_writer(temp.path())
        .expect("retain publication root");
    let _ = initialize_governance_publication_authority_if_pristine(temp.path(), &root_guard)
        .expect("initialize typed publication authority before immutable artifacts");

    let first = write_car_segment_source_fixture(temp.path(), b"publication-a");
    let (first_index, first_entry) = update_publish_index(
        temp.path(),
        empty_governance_publish_index(),
        &first.payload_kind,
        &temp.path().join(&first.encoded_path),
        &temp.path().join(&first.json_path),
        &first.encoded_blake3,
        first.encoded_len,
        &first.json_blake3,
        first.json_len,
        JsonMap::new(),
    )
    .expect("prepare publication A index");
    let first_queue = assemble_governance_car_queue(
        temp.path(),
        &root_guard,
        empty_governance_car_queue(),
        &first_entry,
    )
    .expect("prepare publication A CAR");
    let mut first_state = empty_governance_publication_state();
    first_state.insert("publish_index".into(), JsonValue::Object(first_index));
    first_state.insert("car_queue".into(), JsonValue::Object(first_queue));
    commit_governance_publication_state(temp.path(), &root_guard, first_state)
        .expect("commit publication A");

    let predecessor = read_publication_state_fixture(temp.path());
    assert_eq!(
        predecessor.get("generation").and_then(JsonValue::as_u64),
        Some(1)
    );

    let second = write_car_segment_source_fixture(temp.path(), b"publication-b");
    let mut successor = predecessor
        .as_object()
        .expect("publication A authority object")
        .clone();
    let predecessor_index = successor
        .remove("publish_index")
        .and_then(|value| value.as_object().cloned())
        .expect("publication A index");
    let predecessor_queue = successor
        .remove("car_queue")
        .and_then(|value| value.as_object().cloned())
        .expect("publication A CAR queue");
    let (successor_index, second_entry) = update_publish_index(
        temp.path(),
        predecessor_index,
        &second.payload_kind,
        &temp.path().join(&second.encoded_path),
        &temp.path().join(&second.json_path),
        &second.encoded_blake3,
        second.encoded_len,
        &second.json_blake3,
        second.json_len,
        JsonMap::new(),
    )
    .expect("prepare publication B index");
    let successor_queue =
        assemble_governance_car_queue(temp.path(), &root_guard, predecessor_queue, &second_entry)
            .expect("prepare publication B CAR");
    successor.insert("publish_index".into(), JsonValue::Object(successor_index));
    successor.insert("car_queue".into(), JsonValue::Object(successor_queue));

    commit_governance_publication_state_with(
        temp.path(),
        &root_guard,
        successor.clone(),
        |_guard, _path, _bytes| Err(io::Error::other("injected successor commit failure")),
    )
    .expect_err("publication B authoritative swap must fail");
    assert_eq!(
        read_publication_state_fixture(temp.path()),
        predecessor,
        "a failed successor swap must preserve publication A exactly"
    );
    let visible = read_publication_state_fixture(temp.path());
    assert_eq!(
        visible.get("generation").and_then(JsonValue::as_u64),
        Some(1)
    );
    assert_eq!(
        visible
            .get("publish_index")
            .and_then(|index| index.get("entry_count"))
            .and_then(JsonValue::as_u64),
        Some(1)
    );

    commit_governance_publication_state(temp.path(), &root_guard, successor)
        .expect("retry publication B with the exact prepared successor");
    let committed = read_publication_state_fixture(temp.path());
    assert_eq!(
        committed.get("generation").and_then(JsonValue::as_u64),
        Some(2)
    );
    assert_eq!(
        committed
            .get("publish_index")
            .and_then(|index| index.get("entry_count"))
            .and_then(JsonValue::as_u64),
        Some(2)
    );
    validate_governance_publication_state(
        committed
            .as_object()
            .expect("committed publication B state"),
    )
    .expect("publication B commits both nested indexes together");
}

#[test]
fn governance_publication_state_rejects_cross_section_substitution() {
    let temp = tempdir().expect("tempdir");
    let root_guard = GovernanceFilesystemRootGuard::capture_writer(temp.path())
        .expect("retain publication root");
    let fixture = write_car_segment_source_fixture(temp.path(), b"canonical-payload");
    let (publish_index, entry) = update_publish_index(
        temp.path(),
        empty_governance_publish_index(),
        &fixture.payload_kind,
        &temp.path().join(&fixture.encoded_path),
        &temp.path().join(&fixture.json_path),
        &fixture.encoded_blake3,
        fixture.encoded_len,
        &fixture.json_blake3,
        fixture.json_len,
        JsonMap::new(),
    )
    .expect("prepare publish index");
    let mut queue = assemble_governance_car_queue(
        temp.path(),
        &root_guard,
        empty_governance_car_queue(),
        &entry,
    )
    .expect("prepare CAR queue");
    queue
        .get_mut("segments")
        .and_then(JsonValue::as_array_mut)
        .and_then(|segments| segments.first_mut())
        .and_then(JsonValue::as_object_mut)
        .expect("first CAR segment")
        .insert(
            "encoded_len".into(),
            JsonValue::from(u64::try_from(fixture.encoded_len + 1).expect("small fixture")),
        );
    let mut state = empty_governance_publication_state();
    state.insert("publish_index".into(), JsonValue::Object(publish_index));
    state.insert("car_queue".into(), JsonValue::Object(queue));

    let error = validate_governance_publication_state(&state)
        .expect_err("cross-section substitution must fail closed");
    assert!(error.to_string().contains("one-to-one"));
}

#[test]
fn governance_publication_state_rejects_noncanonical_car_artifact_paths() {
    let temp = tempdir().expect("tempdir");
    let root_guard = GovernanceFilesystemRootGuard::capture_writer(temp.path())
        .expect("retain publication root");
    let fixture = write_car_segment_source_fixture(temp.path(), b"canonical-payload");
    let (publish_index, entry) = update_publish_index(
        temp.path(),
        empty_governance_publish_index(),
        &fixture.payload_kind,
        &temp.path().join(&fixture.encoded_path),
        &temp.path().join(&fixture.json_path),
        &fixture.encoded_blake3,
        fixture.encoded_len,
        &fixture.json_blake3,
        fixture.json_len,
        JsonMap::new(),
    )
    .expect("prepare publish index");
    let mut queue = assemble_governance_car_queue(
        temp.path(),
        &root_guard,
        empty_governance_car_queue(),
        &entry,
    )
    .expect("prepare CAR queue");
    queue
        .get_mut("segments")
        .and_then(JsonValue::as_array_mut)
        .and_then(|segments| segments.first_mut())
        .and_then(JsonValue::as_object_mut)
        .expect("first CAR segment")
        .insert(
            "car_path".into(),
            JsonValue::from("car-segments/00000000000000000000_substituted.car"),
        );
    let mut state = empty_governance_publication_state();
    state.insert("publish_index".into(), JsonValue::Object(publish_index));
    state.insert("car_queue".into(), JsonValue::Object(queue));

    let error = validate_governance_publication_state(&state)
        .expect_err("CAR paths must be derived from the exact position/source identity");
    assert!(
        error
            .to_string()
            .contains("canonical composite-identity path")
    );
}

#[cfg(unix)]
#[test]
fn governance_car_segment_sources_reject_linked_path_components() {
    let temp = tempdir().expect("tempdir");
    let mut entry = write_car_segment_source_fixture(temp.path(), b"canonical-payload");
    let root_guard =
        GovernanceFilesystemRootGuard::capture_writer(temp.path()).expect("retain CAR source root");
    let encoded_path = temp.path().join(&entry.encoded_path);
    let source_dir = encoded_path.parent().expect("canonical source directory");
    std::os::unix::fs::symlink(source_dir, temp.path().join("linked"))
        .expect("create linked source directory");
    entry.encoded_path = "linked/payload.to".to_owned();

    governance_car_segment_files(temp.path(), &root_guard, &entry)
        .expect_err("descriptor-rooted CAR reads must reject linked components");
}

#[test]
fn governance_car_segment_source_reader_stays_rooted_and_per_file_bounded() {
    let source = include_str!("../../governance.rs");
    let start = source
        .find("fn governance_car_segment_files(")
        .expect("CAR source reader definition");
    let end = source[start..]
        .find("\nfn governance_car_plan_json(")
        .map(|offset| start + offset)
        .expect("end of CAR source reader definition");
    let reader = &source[start..end];

    assert!(reader.contains("read_rooted_governance_state_file"));
    assert!(reader.contains("entry.encoded_len"));
    assert!(reader.contains("entry.encoded_blake3"));
    assert!(reader.contains("entry.json_len"));
    assert!(reader.contains("entry.json_blake3"));
    assert!(reader.contains("GOVERNANCE_DIGEST_SIDECAR_BYTES"));
    assert!(reader.contains("GOVERNANCE_CAR_SOURCE_JSON_MAX_BYTES"));
    assert!(reader.contains("snapshot.binding().verify()"));
    assert!(reader.contains("digest sidecar does not match retained source bytes"));
    assert!(
        !reader.contains("fs::read("),
        "CAR source reads must remain descriptor-rooted"
    );
}

#[test]
fn governance_dag_head_age_seconds_saturates_for_future_heads() {
    assert_eq!(
        governance_dag_head_age_seconds(1_800_000_000, 1_800_000_045),
        45
    );
    assert_eq!(
        governance_dag_head_age_seconds(1_800_000_100, 1_800_000_045),
        0
    );
}

#[test]
fn governance_dag_head_generated_at_from_index_prefers_head_timestamp() {
    let mut index = JsonMap::new();
    assert_eq!(governance_dag_head_generated_at_from_index(&index), None);

    index.insert("generated_at".into(), JsonValue::from(1_800_000_000u64));
    assert_eq!(
        governance_dag_head_generated_at_from_index(&index),
        Some(1_800_000_000)
    );

    index.insert(
        "head_generated_at".into(),
        JsonValue::from(1_800_000_045u64),
    );
    assert_eq!(
        governance_dag_head_generated_at_from_index(&index),
        Some(1_800_000_045)
    );
}

#[test]
fn bounded_governance_state_reader_rejects_oversized_file() {
    let temp = tempdir().expect("tempdir");
    let path = temp.path().join("index.json");
    fs::write(&path, b"123456789").expect("write oversized state");

    let error = read_bounded_governance_state_file(&path, 8)
        .expect_err("oversized governance state must fail before allocation");
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert!(error.to_string().contains("exceeds 8 bytes"));
}

#[cfg(unix)]
#[test]
fn bounded_governance_state_reader_rejects_symlink() {
    let temp = tempdir().expect("tempdir");
    let target = temp.path().join("target.json");
    let path = temp.path().join("index.json");
    fs::write(&target, b"{}").expect("write target");
    std::os::unix::fs::symlink(&target, &path).expect("create index symlink");

    let error = read_bounded_governance_state_file(&path, 8)
        .expect_err("governance state symlink must fail closed");
    assert!(error.to_string().contains("must not be a symlink"));
}

#[cfg(unix)]
#[test]
fn bounded_governance_state_reader_rejects_hard_link() {
    let temp = tempdir().expect("tempdir");
    let target = temp.path().join("target.json");
    let path = temp.path().join("index.json");
    fs::write(&target, b"{}").expect("write target");
    fs::hard_link(&target, &path).expect("create index hard link");

    let error = read_bounded_governance_state_file(&path, 8)
        .expect_err("hard-linked governance state must fail closed");
    assert!(error.to_string().contains("exactly one hard link"));
}

struct TestRuntimeDagSigner {
    handle: String,
    publisher_peer_id: Vec<u8>,
    key_pair: KeyPair,
    public_key_override: Option<[u8; 32]>,
    qualification_revision: AtomicU64,
    qualification_reads: AtomicU64,
    drift_on_second_qualification_read: AtomicBool,
    qualification_error: Option<String>,
    drift_during_sign: AtomicBool,
    refuse_with: Option<String>,
    corrupt_signature: bool,
    last_purpose: Mutex<Option<GovernanceDagSigningPurposeV1>>,
}

impl fmt::Debug for TestRuntimeDagSigner {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TestRuntimeDagSigner")
            .field("handle", &self.handle)
            .field("publisher_peer_id", &self.publisher_peer_id)
            .finish_non_exhaustive()
    }
}

impl TestRuntimeDagSigner {
    fn new(handle: &str, publisher_peer_id: &[u8], seed: u8) -> Self {
        Self {
            handle: handle.to_owned(),
            publisher_peer_id: publisher_peer_id.to_vec(),
            key_pair: KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                .expect("derive test runtime DAG signer"),
            public_key_override: None,
            qualification_revision: AtomicU64::new(1),
            qualification_reads: AtomicU64::new(0),
            drift_on_second_qualification_read: AtomicBool::new(false),
            qualification_error: None,
            drift_during_sign: AtomicBool::new(false),
            refuse_with: None,
            corrupt_signature: false,
            last_purpose: Mutex::new(None),
        }
    }

    fn public_key_bytes(&self) -> [u8; 32] {
        let (algorithm, bytes) = self
            .key_pair
            .public_key()
            .try_to_bytes()
            .expect("serialize test public key");
        assert_eq!(algorithm, Algorithm::Ed25519);
        bytes.try_into().expect("Ed25519 public key is fixed-width")
    }

    fn observed_purpose(&self) -> Option<GovernanceDagSigningPurposeV1> {
        *self.last_purpose.lock().expect("signing purpose lock")
    }
}

impl GovernanceDagRuntimeSigner for TestRuntimeDagSigner {
    fn handle(&self) -> &str {
        &self.handle
    }

    fn qualification(&self) -> Result<GovernanceDagRuntimeProviderQualificationV1, String> {
        if let Some(error) = &self.qualification_error {
            return Err(error.clone());
        }
        let read_index = self.qualification_reads.fetch_add(1, Ordering::SeqCst);
        let revision = self.qualification_revision.load(Ordering::SeqCst);
        let revision = if self
            .drift_on_second_qualification_read
            .load(Ordering::SeqCst)
            && read_index == 1
        {
            revision.saturating_add(1)
        } else {
            revision
        };
        Ok(GovernanceDagRuntimeProviderQualificationV1::new(
            revision,
            TEST_RUNTIME_DAG_SIGNER_POLICY_DIGEST,
        ))
    }

    fn publisher_peer_id(&self) -> &[u8] {
        &self.publisher_peer_id
    }

    fn public_key(&self) -> [u8; 32] {
        self.public_key_override
            .unwrap_or_else(|| self.public_key_bytes())
    }

    fn sign(
        &self,
        purpose: GovernanceDagSigningPurposeV1,
        payload: &[u8],
    ) -> Result<[u8; 64], String> {
        *self.last_purpose.lock().expect("signing purpose lock") = Some(purpose);
        if self.drift_during_sign.swap(false, Ordering::SeqCst) {
            self.qualification_revision.fetch_add(1, Ordering::SeqCst);
        }
        if let Some(error) = &self.refuse_with {
            return Err(error.clone());
        }
        let mut signature: [u8; 64] = IrohaSignature::try_new(self.key_pair.private_key(), payload)
            .expect("test runtime signer can sign")
            .payload()
            .try_into()
            .expect("Ed25519 signature is fixed-width");
        if self.corrupt_signature {
            signature[0] ^= 0x80;
        }
        Ok(signature)
    }
}

fn qualified_test_runtime_dag_signer(revision: u64, seed: u8) -> GovernanceRuntimeDagSigner {
    let peer_id = b"12D3KooWRuntimeDagPublisher".to_vec();
    let signer = Arc::new(TestRuntimeDagSigner::new(
        "pkcs11:governance-dag:primary",
        &peer_id,
        seed,
    ));
    signer
        .qualification_revision
        .store(revision, Ordering::SeqCst);
    let public_key = signer.public_key();
    GovernanceRuntimeDagSigner::try_new(
        "pkcs11:governance-dag:primary".to_owned(),
        peer_id,
        public_key,
        GovernanceDagRuntimeProviderQualificationV1::new(
            revision,
            TEST_RUNTIME_DAG_SIGNER_POLICY_DIGEST,
        ),
        signer,
    )
    .expect("qualify runtime DAG signer")
}

fn qualified_test_runtime_dag_checkpoint_store(
    store: Arc<TestRuntimeDagCheckpointStore>,
) -> GovernanceRuntimeDagCheckpointStore {
    GovernanceRuntimeDagCheckpointStore::try_new(
        TestRuntimeDagCheckpointStore::HANDLE.to_owned(),
        TestRuntimeDagCheckpointStore::qualification(),
        store,
    )
    .expect("qualify runtime DAG checkpoint store")
}

fn signed_runtime_publisher_with_store(
    root: &Path,
    store: Arc<TestRuntimeDagCheckpointStore>,
) -> FilesystemGovernancePublisher {
    FilesystemGovernancePublisher::try_new(root.to_path_buf())
        .expect("publisher")
        .with_qualified_runtime_dag_providers(
            qualified_test_runtime_dag_signer(1, 0x31),
            qualified_test_runtime_dag_checkpoint_store(store),
        )
        .expect("runtime DAG providers")
}

fn signed_runtime_publisher(root: &Path) -> FilesystemGovernancePublisher {
    signed_runtime_publisher_with_store(root, Arc::new(TestRuntimeDagCheckpointStore::default()))
}

fn signed_runtime_publisher_with_observable_providers(
    root: &Path,
) -> (
    FilesystemGovernancePublisher,
    Arc<TestRuntimeDagSigner>,
    Arc<TestRuntimeDagCheckpointStore>,
) {
    let peer_id = b"12D3KooWRuntimeDagPublisher".to_vec();
    let signer_provider = Arc::new(TestRuntimeDagSigner::new(
        "pkcs11:governance-dag:primary",
        &peer_id,
        0x31,
    ));
    let signer = GovernanceRuntimeDagSigner::try_new(
        "pkcs11:governance-dag:primary".to_owned(),
        peer_id,
        signer_provider.public_key(),
        test_runtime_dag_signer_qualification(),
        signer_provider.clone(),
    )
    .expect("qualify observable runtime DAG signer");
    let checkpoint_provider = Arc::new(TestRuntimeDagCheckpointStore::default());
    let checkpoint_store =
        qualified_test_runtime_dag_checkpoint_store(Arc::clone(&checkpoint_provider));
    let publisher = FilesystemGovernancePublisher::try_new(root.to_path_buf())
        .expect("publisher")
        .with_qualified_runtime_dag_providers(signer, checkpoint_store)
        .expect("runtime DAG providers");
    (publisher, signer_provider, checkpoint_provider)
}

fn runtime_index(root: &Path) -> JsonValue {
    let root_guard = GovernanceFilesystemRootGuard::capture_writer(root)
        .expect("retain runtime DAG fixture root");
    let store = open_runtime_dag_committed_store_v1(root, &root_guard)
        .expect("open runtime DAG committed fixture store");
    let (state, _) =
        load_runtime_dag_committed_state_v1(&store).expect("load committed fixture state");
    let bytes = state.index_bytes.expect("runtime index exists");
    let index: JsonValue = norito::json::from_slice(&bytes).expect("runtime index parses");
    assert_eq!(
        index.get("root").and_then(JsonValue::as_str),
        Some(GOVERNANCE_DAG_LOGICAL_ROOT),
        "public runtime index must not disclose its host filesystem root"
    );
    index
}

fn runtime_head_bytes(root: &Path) -> Vec<u8> {
    let root_guard = GovernanceFilesystemRootGuard::capture_writer(root)
        .expect("retain runtime DAG fixture root");
    let store = open_runtime_dag_committed_store_v1(root, &root_guard)
        .expect("open runtime DAG committed fixture store");
    load_runtime_dag_committed_state_v1(&store)
        .expect("load committed fixture state")
        .0
        .head_bytes
        .expect("runtime head exists")
}

fn runtime_blocks_from_index(root: &Path, index: &JsonValue) -> Vec<GovernanceDagBlockV1> {
    index
        .get("blocks")
        .and_then(JsonValue::as_array)
        .expect("runtime blocks")
        .iter()
        .map(|entry| {
            let block_path = entry
                .get("block_path")
                .and_then(JsonValue::as_str)
                .expect("block path");
            let block_path = resolve_index_path(root, block_path).expect("resolve block path");
            let bytes = fs::read(block_path).expect("read runtime block");
            norito::decode_from_bytes(&bytes).expect("decode runtime block")
        })
        .collect()
}

fn filesystem_inventory_fixture(root: &Path) -> Vec<(PathBuf, bool, u64)> {
    fn visit(root: &Path, directory: &Path, inventory: &mut Vec<(PathBuf, bool, u64)>) {
        let mut entries = fs::read_dir(directory)
            .expect("enumerate fixture directory")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect fixture directory");
        entries.sort_by_key(fs::DirEntry::file_name);
        for entry in entries {
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path).expect("read fixture metadata");
            let relative = path
                .strip_prefix(root)
                .expect("fixture entry remains below root")
                .to_path_buf();
            inventory.push((relative, metadata.is_dir(), metadata.len()));
            if metadata.is_dir() {
                visit(root, &path, inventory);
            }
        }
    }

    let mut inventory = Vec::new();
    visit(root, root, &mut inventory);
    inventory
}

fn assert_single_runtime_external(root: &Path, kind: &str, encoded: &[u8]) {
    let index = runtime_index(root);
    let blocks = runtime_blocks_from_index(root, &index);
    assert_eq!(blocks.len(), 1);
    match &blocks[0].node.payload {
        GovernanceLogPayloadV1::ExternalPayload(payload) => {
            payload.validate().expect("external payload validates");
            assert_eq!(payload.payload_kind, kind);
            assert_eq!(payload.encoded_payload, encoded);
            assert_eq!(payload.encoded_blake3, *blake3::hash(encoded).as_bytes());
        }
        other => panic!("expected external runtime payload, found {other:?}"),
    }
}

#[test]
fn filesystem_publisher_rejects_noncanonical_or_mismatched_payload_bytes() {
    let temp = tempdir().expect("tempdir");
    let publisher =
        FilesystemGovernancePublisher::try_new(temp.path().to_path_buf()).expect("publisher");
    let (settlement, canonical) = sample_settlement();

    let bare = settlement.encode();
    let error = publisher
        .publish_deal_settlement(&settlement, &bare)
        .expect_err("bare payload without a Norito header must fail");
    assert!(error.to_string().contains("canonical header-bearing"));

    let mut conflicting = settlement.clone();
    conflicting.audit_notes = Some("different typed payload".to_owned());
    let error = publisher
        .publish_deal_settlement(&conflicting, &canonical)
        .expect_err("typed payload and canonical bytes must match");
    assert!(error.to_string().contains("do not match"));
    assert!(
        !temp
            .path()
            .join(GOVERNANCE_PUBLICATION_SOURCES_DIR)
            .exists(),
        "validation must fail before any governance artifact is written"
    );
}

#[test]
fn filesystem_publisher_rejects_semantically_invalid_payload_before_writes() {
    let temp = tempdir().expect("tempdir");
    let publisher =
        FilesystemGovernancePublisher::try_new(temp.path().to_path_buf()).expect("publisher");
    let (mut settlement, _) = sample_settlement();
    settlement.deal_id[0] ^= 0x80;
    let encoded = norito::to_bytes(&settlement).expect("encode invalid settlement");

    let error = publisher
        .publish_deal_settlement(&settlement, &encoded)
        .expect_err("ledger and settlement deal identifiers must match");
    assert!(error.to_string().contains("invalid deal settlement"));
    assert!(
        !temp
            .path()
            .join(GOVERNANCE_PUBLICATION_SOURCES_DIR)
            .exists(),
        "semantic validation must fail before any governance artifact is written"
    );
}

#[test]
fn filesystem_publisher_writes_por_payloads_into_one_signed_canonical_chain() {
    let temp = tempdir().expect("tempdir");
    let publisher = signed_runtime_publisher(temp.path());
    let (publication, publication_encoded) = sample_por_challenge_publication();
    let (report, report_encoded) = sample_por_weekly_report();

    publisher
        .publish_por_challenge_publication(&publication, &publication_encoded)
        .expect("publish PoR challenge");
    publisher
        .publish_por_weekly_report(&report, &report_encoded)
        .expect("publish PoR weekly report");

    let challenge_path = temp
        .path()
        .join("por")
        .join("challenges")
        .join(format!("{:020}", publication.challenge.epoch_id))
        .join(hex::encode(publication.challenge.challenge_id))
        .with_extension("to");
    assert_eq!(
        fs::read(&challenge_path).expect("read canonical challenge publication"),
        publication_encoded
    );

    let report_digest = blake3::hash(&report_encoded).to_hex().to_string();
    let report_path = temp
        .path()
        .join("por")
        .join("reports")
        .join(format!(
            "{:04}-W{:02}_{:020}_{}",
            report.cycle.year,
            report.cycle.week,
            report.generated_at,
            &report_digest[..16],
        ))
        .with_extension("to");
    assert_eq!(
        fs::read(&report_path).expect("read canonical weekly report"),
        report_encoded
    );

    let index = runtime_index(temp.path());
    let blocks = runtime_blocks_from_index(temp.path(), &index);
    assert_eq!(blocks.len(), 2);
    assert_eq!(blocks[1].prev_block_cid, Some(blocks[0].block_cid.clone()));
    assert_eq!(
        blocks[1].node.prev_cid,
        Some(blocks[0].node.node_cid.clone())
    );
    assert_eq!(
        blocks[0].node.payload,
        GovernanceLogPayloadV1::PorChallengePublication(publication)
    );
    assert_eq!(
        blocks[1].node.payload,
        GovernanceLogPayloadV1::PorWeeklyReport(report)
    );
    let head_bytes = runtime_head_bytes(temp.path());
    let head: GovernanceDagHeadV1 =
        norito::decode_from_bytes(&head_bytes).expect("decode signed runtime head");
    validate_governance_dag_head_against_chain_v1(&head, &blocks)
        .expect("PoR runtime chain and head validate");
}

#[test]
fn filesystem_publisher_root_has_a_single_process_owner() {
    let temp = tempdir().expect("tempdir");
    let owner = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
        .expect("acquire publisher root");

    let error = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
        .expect_err("a second publisher must not share mutable index state");
    assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
    assert!(error.to_string().contains("already in use"));

    drop(owner);
    FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
        .expect("publisher root ownership releases on drop");
}

#[test]
fn filesystem_publisher_restart_rejects_runtime_signer_revision_substitution() {
    let temp = tempdir().expect("tempdir");
    let checkpoint_store = Arc::new(TestRuntimeDagCheckpointStore::default());
    {
        let publisher =
            signed_runtime_publisher_with_store(temp.path(), Arc::clone(&checkpoint_store));
        let (settlement, encoded) = sample_settlement();
        publisher
            .publish_deal_settlement(&settlement, &encoded)
            .expect("seed signed runtime DAG");
    }
    let index = runtime_index(temp.path());
    assert_eq!(
        index.get("signer_revision").and_then(JsonValue::as_u64),
        Some(1)
    );
    let expected_policy_digest = hex::encode(TEST_RUNTIME_DAG_SIGNER_POLICY_DIGEST);
    assert_eq!(
        index
            .get("signer_policy_digest_hex")
            .and_then(JsonValue::as_str),
        Some(expected_policy_digest.as_str())
    );

    let peer_id = b"12D3KooWRuntimeDagPublisher".to_vec();
    let provider = Arc::new(TestRuntimeDagSigner::new(
        "pkcs11:governance-dag:primary",
        &peer_id,
        0x31,
    ));
    provider.qualification_revision.store(2, Ordering::SeqCst);
    let signer = GovernanceRuntimeDagSigner::try_new(
        "pkcs11:governance-dag:primary".to_owned(),
        peer_id,
        provider.public_key(),
        GovernanceDagRuntimeProviderQualificationV1::new(2, TEST_RUNTIME_DAG_SIGNER_POLICY_DIGEST),
        provider,
    )
    .expect("qualify rotated runtime signer");
    let error = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
        .expect("reopen publisher root")
        .with_qualified_runtime_dag_providers(
            signer,
            qualified_test_runtime_dag_checkpoint_store(checkpoint_store),
        )
        .expect_err("implicit signer revision rotation must fail startup");
    assert!(
        error.to_string().contains("malformed")
            || error.to_string().contains("another root or signer")
            || error.to_string().contains("provider binding")
    );
}

#[test]
fn filesystem_publisher_replays_authenticated_provider_transition_after_ambiguous_cas() {
    let temp = tempdir().expect("tempdir");
    let checkpoint_store = Arc::new(TestRuntimeDagCheckpointStore::default());
    let mut publisher =
        signed_runtime_publisher_with_store(temp.path(), Arc::clone(&checkpoint_store));
    let (settlement, encoded) = sample_settlement();
    publisher
        .publish_deal_settlement(&settlement, &encoded)
        .expect("seed signed runtime DAG");

    checkpoint_store
        .fail_after_next_checkpoint_cas
        .store(true, Ordering::SeqCst);
    let next_store = publisher
        .runtime_dag_checkpoint_store
        .as_ref()
        .expect("checkpoint store")
        .clone();
    let error = publisher
        .transition_qualified_runtime_dag_providers(
            qualified_test_runtime_dag_signer(2, 0x31),
            next_store,
        )
        .expect_err("ambiguous provider-transition checkpoint CAS must surface");
    assert!(error.to_string().contains("compare-and-swap failed"));
    drop(publisher);

    let recovered = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
        .expect("reopen publisher")
        .with_qualified_runtime_dag_providers(
            qualified_test_runtime_dag_signer(2, 0x31),
            qualified_test_runtime_dag_checkpoint_store(Arc::clone(&checkpoint_store)),
        )
        .expect("replay exact signed provider transition");
    let index = runtime_index(temp.path());
    assert_eq!(
        index.get("signer_revision").and_then(JsonValue::as_u64),
        Some(2)
    );
    let binding = runtime_dag_provider_binding(
        recovered
            .runtime_dag_signer
            .as_ref()
            .expect("recovered signer"),
        recovered
            .runtime_dag_checkpoint_store
            .as_ref()
            .expect("recovered store"),
    );
    let (_, summary) =
        read_runtime_dag_qualification_history(temp.path(), recovered.root_guard(), Some(&binding))
            .expect("read transition history")
            .expect("transition history exists");
    assert_eq!(summary.transition_generation, 1);
    assert_ne!(summary.transition_digest, [0; 32]);
    drop(recovered);
}

#[test]
fn filesystem_publisher_rotates_signing_keys_with_authenticated_authority_segments() {
    let temp = tempdir().expect("tempdir");
    let checkpoint_store = Arc::new(TestRuntimeDagCheckpointStore::default());
    let initial_signer = qualified_test_runtime_dag_signer(1, 0x31);
    let initial_public_key = initial_signer.public_key;
    let mut publisher = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
        .expect("publisher")
        .with_qualified_runtime_dag_providers(
            initial_signer,
            qualified_test_runtime_dag_checkpoint_store(Arc::clone(&checkpoint_store)),
        )
        .expect("initial runtime DAG providers");
    let (settlement, settlement_encoded) = sample_settlement();
    publisher
        .publish_deal_settlement(&settlement, &settlement_encoded)
        .expect("publish under the outgoing authority");

    let next_signer = qualified_test_runtime_dag_signer(2, 0x32);
    let next_public_key = next_signer.public_key;
    assert_ne!(initial_public_key, next_public_key);
    let next_store = publisher
        .runtime_dag_checkpoint_store
        .as_ref()
        .expect("checkpoint store")
        .clone();
    publisher
        .transition_qualified_runtime_dag_providers(next_signer, next_store)
        .expect("rotate to a distinct authenticated signing key");

    let current_signer = publisher
        .runtime_dag_signer
        .as_ref()
        .expect("rotated signer");
    let current_store = publisher
        .runtime_dag_checkpoint_store
        .as_ref()
        .expect("rotated checkpoint store");
    validate_existing_runtime_dag_root(temp.path(), current_signer, current_store)
        .expect("an outgoing-signed tip remains valid until the incoming key appends");
    let rotated_snapshot = load_authenticated_runtime_dag_snapshot_v1(
        publisher.root_guard(),
        current_signer,
        current_store,
    )
    .expect("strict reader accepts the current provider binding after rotation")
    .expect("rotated one-block DAG has a committed snapshot");
    let rotated_head: GovernanceDagHeadV1 =
        norito::decode_from_bytes(rotated_snapshot.head_bytes())
            .expect("decode outgoing-signed rotated head");
    assert_eq!(
        rotated_head.head_signature.public_key,
        initial_public_key.to_vec(),
        "the strict reader must accept an outgoing-signed tip under the current provider binding"
    );
    let current_binding = runtime_dag_provider_binding(current_signer, current_store);
    let lineage = runtime_dag_authority_lineage(temp.path(), &current_binding)
        .expect("read authenticated authority lineage");
    assert_eq!(lineage.segments.len(), 2);
    assert_eq!(lineage.transitions.len(), 1);
    assert_eq!(lineage.segments[0].activation_block_count, 0);
    assert_eq!(lineage.segments[0].revision, 1);
    assert_eq!(
        lineage.segments[0].binding.publisher_public_key,
        initial_public_key
    );
    assert_eq!(lineage.segments[1].activation_block_count, 1);
    assert_eq!(lineage.segments[1].revision, 2);
    assert_eq!(
        lineage.segments[1].binding.publisher_public_key,
        next_public_key
    );
    validate_runtime_dag_qualification_transition(
        &lineage.transitions[0],
        runtime_dag_producer_root_digest(temp.path()).expect("root digest"),
    )
    .expect("both continuity signatures authenticate the key transition");

    let (publication, publication_encoded) = sample_por_challenge_publication();
    publisher
        .publish_por_challenge_publication(&publication, &publication_encoded)
        .expect("publish under the incoming authority");
    let index = runtime_index(temp.path());
    let blocks = runtime_blocks_from_index(temp.path(), &index);
    assert_eq!(blocks.len(), 2);
    assert_eq!(
        blocks[0].block_signature.public_key,
        initial_public_key.to_vec()
    );
    assert_eq!(
        blocks[1].block_signature.public_key,
        next_public_key.to_vec()
    );
    let head_bytes = runtime_head_bytes(temp.path());
    let head: GovernanceDagHeadV1 =
        norito::decode_from_bytes(&head_bytes).expect("decode rotated signed head");
    assert_eq!(head.head_signature.public_key, next_public_key.to_vec());
    validate_existing_runtime_dag_root(temp.path(), current_signer, current_store)
        .expect("segmented chain validates after the incoming key appends");
    let incoming_snapshot = load_authenticated_runtime_dag_snapshot_v1(
        publisher.root_guard(),
        current_signer,
        current_store,
    )
    .expect("strict reader accepts the incoming authority append")
    .expect("two-block rotated DAG has a committed snapshot");
    let incoming_head: GovernanceDagHeadV1 =
        norito::decode_from_bytes(incoming_snapshot.head_bytes())
            .expect("decode incoming-signed rotated head");
    assert_eq!(
        incoming_head.head_signature.public_key,
        next_public_key.to_vec()
    );
    drop(publisher);

    let recovered = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
        .expect("reopen publisher after key rotation")
        .with_qualified_runtime_dag_providers(
            qualified_test_runtime_dag_signer(2, 0x32),
            qualified_test_runtime_dag_checkpoint_store(Arc::clone(&checkpoint_store)),
        )
        .expect("recover using only the current runtime providers");
    validate_existing_runtime_dag_root(
        temp.path(),
        recovered
            .runtime_dag_signer
            .as_ref()
            .expect("recovered signer"),
        recovered
            .runtime_dag_checkpoint_store
            .as_ref()
            .expect("recovered checkpoint store"),
    )
    .expect("bounded recovery authenticates every historical signer segment");
}

#[test]
fn qualification_compaction_seals_archive_before_prune_and_recovers_idempotently() {
    let temp = tempdir().expect("tempdir");
    let checkpoint_store = Arc::new(TestRuntimeDagCheckpointStore::default());
    let mut publisher =
        signed_runtime_publisher_with_store(temp.path(), Arc::clone(&checkpoint_store));
    let (settlement, encoded) = sample_settlement();
    publisher
        .publish_deal_settlement(&settlement, &encoded)
        .expect("seed signed runtime DAG");
    for revision in 2..=4 {
        let next_store = publisher
            .runtime_dag_checkpoint_store
            .as_ref()
            .expect("checkpoint store")
            .clone();
        publisher
            .transition_qualified_runtime_dag_providers(
                qualified_test_runtime_dag_signer(revision, 0x31),
                next_store,
            )
            .expect("append provider transition");
    }

    checkpoint_store
        .fail_before_next_checkpoint_cas
        .store(true, Ordering::SeqCst);
    let error = publisher
        .compact_runtime_dag_qualification_history(1)
        .expect_err("archive checkpoint refusal must surface after durable archive install");
    assert!(error.to_string().contains("compare-and-swap failed"));
    drop(publisher);

    let mut recovered = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
        .expect("reopen publisher")
        .with_qualified_runtime_dag_providers(
            qualified_test_runtime_dag_signer(4, 0x31),
            qualified_test_runtime_dag_checkpoint_store(Arc::clone(&checkpoint_store)),
        )
        .expect("finish archive prune from sealed checkpoint");
    let binding = runtime_dag_provider_binding(
        recovered
            .runtime_dag_signer
            .as_ref()
            .expect("recovered signer"),
        recovered
            .runtime_dag_checkpoint_store
            .as_ref()
            .expect("recovered store"),
    );
    let (history, summary) =
        read_runtime_dag_qualification_history(temp.path(), recovered.root_guard(), Some(&binding))
            .expect("read compacted history")
            .expect("compacted history exists");
    assert_eq!(history.transitions.len(), 1);
    assert_eq!(history.archived_through_generation, 2);
    assert_eq!(summary.transition_generation, 3);
    assert_eq!(summary.archive_generation, 1);
    assert_ne!(summary.archive_digest, [0; 32]);
    let archive_path = runtime_dag_qualification_archive_path(
        temp.path(),
        summary.archive_generation,
        summary.archive_digest,
    );
    fs::remove_file(digest_sidecar_path_for(&archive_path))
        .expect("simulate crash before archive sidecar install");
    let archive = read_runtime_dag_qualification_archive(
        temp.path(),
        summary.archive_generation,
        summary.archive_digest,
        history.root_digest,
    )
    .expect("read signed qualification archive");
    assert!(
        digest_sidecar_path_for(&archive_path).is_file(),
        "authenticated archive replay restores its missing sidecar"
    );
    let mut tampered_archive = archive;
    tampered_archive.signature[0] ^= 0x80;
    assert!(
        validate_runtime_dag_qualification_archive(&tampered_archive, history.root_digest,)
            .is_err()
    );
    assert_eq!(
        recovered
            .compact_runtime_dag_qualification_history(1)
            .expect("idempotent compaction replay"),
        0
    );
    for revision in 5..=6 {
        let next_store = recovered
            .runtime_dag_checkpoint_store
            .as_ref()
            .expect("checkpoint store")
            .clone();
        recovered
            .transition_qualified_runtime_dag_providers(
                qualified_test_runtime_dag_signer(revision, 0x31),
                next_store,
            )
            .expect("append post-archive provider transition");
    }
    checkpoint_store
        .fail_after_next_checkpoint_cas
        .store(true, Ordering::SeqCst);
    let error = recovered
        .compact_runtime_dag_qualification_history(1)
        .expect_err("ambiguous post-CAS archive checkpoint response must surface");
    assert!(error.to_string().contains("compare-and-swap failed"));
    drop(recovered);

    let recovered = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
        .expect("reopen publisher after post-CAS crash")
        .with_qualified_runtime_dag_providers(
            qualified_test_runtime_dag_signer(6, 0x31),
            qualified_test_runtime_dag_checkpoint_store(Arc::clone(&checkpoint_store)),
        )
        .expect("finish post-CAS archive prune");
    let binding = runtime_dag_provider_binding(
        recovered
            .runtime_dag_signer
            .as_ref()
            .expect("recovered signer"),
        recovered
            .runtime_dag_checkpoint_store
            .as_ref()
            .expect("recovered store"),
    );
    let (history, summary) =
        read_runtime_dag_qualification_history(temp.path(), recovered.root_guard(), Some(&binding))
            .expect("read twice-compacted history")
            .expect("twice-compacted history exists");
    assert_eq!(history.transitions.len(), 1);
    assert_eq!(summary.transition_generation, 5);
    assert_eq!(summary.archive_generation, 2);
    assert_eq!(
        recovered
            .compact_runtime_dag_qualification_history(1)
            .expect("second idempotent compaction replay"),
        0
    );
    drop(recovered);
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn qualification_archive_crash_temp_is_quarantined_before_history_inventory() {
    let temp = tempdir().expect("tempdir");
    let checkpoint_store = Arc::new(TestRuntimeDagCheckpointStore::default());
    let publisher = signed_runtime_publisher_with_store(temp.path(), Arc::clone(&checkpoint_store));
    let (settlement, encoded) = sample_settlement();
    publisher
        .publish_deal_settlement(&settlement, &encoded)
        .expect("seed signed runtime DAG");

    let checkpoint_record = checkpoint_store
        .load(GovernanceDagSealedStateSlot::ProducerCheckpoint)
        .expect("load producer checkpoint")
        .expect("producer checkpoint exists");
    let checkpoint =
        decode_runtime_dag_unqualified_checkpoint_record(&checkpoint_record, temp.path())
            .expect("decode producer checkpoint");
    let next_generation = checkpoint
        .qualification_archive_generation
        .checked_add(1)
        .expect("test archive generation");
    let archive_path =
        runtime_dag_qualification_archive_path(temp.path(), next_generation, [0xA7; 32]);
    fs::create_dir_all(archive_path.parent().expect("archive parent"))
        .expect("create qualification archive directory");
    let crash_temp = temp_path_for_atomic(&archive_path, 42_000, 7);
    fs::write(&crash_temp, b"crash-before-archive-rename")
        .expect("seed qualification archive crash temp");
    fs::set_permissions(&crash_temp, fs::Permissions::from_mode(0o600))
        .expect("make archive crash temp private");

    let error = recover_runtime_dag_qualification_compaction(
        temp.path(),
        publisher.root_guard(),
        publisher
            .runtime_dag_signer
            .as_ref()
            .expect("runtime DAG signer"),
        publisher
            .runtime_dag_checkpoint_store
            .as_ref()
            .expect("runtime DAG checkpoint store"),
    )
    .expect_err("recovery must quarantine the archive temp before reading history");
    assert!(
        error
            .to_string()
            .contains(GOVERNANCE_PUBLICATION_RECOVERY_QUARANTINE_DIR),
        "unexpected archive-temp recovery error: {error}"
    );
    assert!(
        !crash_temp.exists(),
        "the crash temp must leave the live archive namespace"
    );
    assert_eq!(
        fs::read_dir(recovery_quarantine_path(temp.path()))
            .expect("read archive recovery quarantine")
            .count(),
        1,
        "the exact interrupted archive object must be retained offline"
    );

    drop(publisher);
    clear_recovery_quarantine_offline(temp.path());
    let recovered = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
        .expect("reopen publisher after offline archive-temp cleanup")
        .with_qualified_runtime_dag_providers(
            qualified_test_runtime_dag_signer(1, 0x31),
            qualified_test_runtime_dag_checkpoint_store(checkpoint_store),
        )
        .expect("archive-temp recovery converges after offline cleanup");
    drop(recovered);
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn qualification_recovery_preserves_canonical_archive_for_history_validation() {
    let temp = tempdir().expect("tempdir");
    let checkpoint_store = Arc::new(TestRuntimeDagCheckpointStore::default());
    let publisher = signed_runtime_publisher_with_store(temp.path(), checkpoint_store);
    let (settlement, encoded) = sample_settlement();
    publisher
        .publish_deal_settlement(&settlement, &encoded)
        .expect("seed signed runtime DAG");
    let archive_path = runtime_dag_qualification_archive_path(temp.path(), 1, [0xA8; 32]);
    fs::create_dir_all(archive_path.parent().expect("archive parent"))
        .expect("create qualification archive directory");
    fs::write(
        &archive_path,
        b"canonical-name-awaiting-authenticated-history",
    )
    .expect("seed canonical archive entry");
    fs::set_permissions(&archive_path, fs::Permissions::from_mode(0o600))
        .expect("make archive entry private");

    let error = recover_runtime_dag_qualification_compaction(
        temp.path(),
        publisher.root_guard(),
        publisher
            .runtime_dag_signer
            .as_ref()
            .expect("runtime signer"),
        publisher
            .runtime_dag_checkpoint_store
            .as_ref()
            .expect("runtime checkpoint store"),
    )
    .expect_err("canonical archive without authenticated history must fail closed");
    assert!(
        error
            .to_string()
            .contains("archives exist without their authenticated history head")
    );
    assert_eq!(
        fs::read(&archive_path).expect("canonical archive entry remains"),
        b"canonical-name-awaiting-authenticated-history"
    );
}

#[test]
fn qualification_history_rejects_tamper_fork_duplicate_rollback_and_bad_bytes() {
    let temp = tempdir().expect("tempdir");
    let checkpoint_store = Arc::new(TestRuntimeDagCheckpointStore::default());
    let mut publisher =
        signed_runtime_publisher_with_store(temp.path(), Arc::clone(&checkpoint_store));
    let (settlement, encoded) = sample_settlement();
    publisher
        .publish_deal_settlement(&settlement, &encoded)
        .expect("seed signed runtime DAG");
    for revision in 2..=4 {
        let next_store = publisher
            .runtime_dag_checkpoint_store
            .as_ref()
            .expect("checkpoint store")
            .clone();
        publisher
            .transition_qualified_runtime_dag_providers(
                qualified_test_runtime_dag_signer(revision, 0x31),
                next_store,
            )
            .expect("append provider transition");
    }
    let binding = runtime_dag_provider_binding(
        publisher
            .runtime_dag_signer
            .as_ref()
            .expect("current signer"),
        publisher
            .runtime_dag_checkpoint_store
            .as_ref()
            .expect("current store"),
    );
    let (history, _) =
        read_runtime_dag_qualification_history(temp.path(), publisher.root_guard(), Some(&binding))
            .expect("read qualification history")
            .expect("qualification history exists");
    assert_eq!(history.transitions.len(), 3);
    let history_path = runtime_dag_qualification_history_path(temp.path());
    assert!(
        !history_path.exists() && !digest_sidecar_path_for(&history_path).exists(),
        "qualification history is authoritative only inside the typed store"
    );
    let qualification_store =
        open_runtime_dag_qualification_store_v1(temp.path(), publisher.root_guard())
            .expect("open typed qualification store");
    let stored_history = load_runtime_dag_qualification_state_v1(&qualification_store)
        .expect("load typed qualification history")
        .0
        .history
        .expect("typed qualification history exists");
    assert_eq!(stored_history.transitions, history.transitions);

    let mut tampered = history.clone();
    tampered.transitions[1].key_transition.incoming_signature[0] ^= 0x80;
    assert!(
        validate_runtime_dag_qualification_history(temp.path(), &tampered, Some(&binding), None,)
            .is_err()
    );

    let mut outgoing_tampered = history.clone();
    outgoing_tampered.transitions[1]
        .key_transition
        .outgoing_signature[0] ^= 0x80;
    assert!(
        validate_runtime_dag_qualification_history(
            temp.path(),
            &outgoing_tampered,
            Some(&binding),
            None,
        )
        .is_err()
    );

    let mut segment_revision_rollback = history.clone();
    let outgoing_revision = segment_revision_rollback.transitions[1]
        .key_transition
        .outgoing_segment_revision;
    segment_revision_rollback.transitions[1]
        .key_transition
        .incoming_segment_revision = outgoing_revision;
    assert!(
        validate_runtime_dag_qualification_history(
            temp.path(),
            &segment_revision_rollback,
            Some(&binding),
            None,
        )
        .is_err()
    );

    let mut replayed_envelope = history.clone();
    replayed_envelope.transitions[1].key_transition =
        replayed_envelope.transitions[0].key_transition.clone();
    assert!(
        validate_runtime_dag_qualification_history(
            temp.path(),
            &replayed_envelope,
            Some(&binding),
            None,
        )
        .is_err()
    );

    let mut forked = history.clone();
    forked.transitions.swap(1, 2);
    assert!(
        validate_runtime_dag_qualification_history(temp.path(), &forked, Some(&binding), None,)
            .is_err()
    );

    let mut duplicated = history.clone();
    duplicated
        .transitions
        .insert(1, duplicated.transitions[0].clone());
    assert!(
        validate_runtime_dag_qualification_history(temp.path(), &duplicated, Some(&binding), None,)
            .is_err()
    );

    let mut rolled_back = history.clone();
    rolled_back.transitions.pop();
    assert!(
        validate_runtime_dag_qualification_history(
            temp.path(),
            &rolled_back,
            Some(&binding),
            None,
        )
        .is_err()
    );

    let mut substituted = history.clone();
    substituted.transitions[2].body.next.signer_revision += 1;
    assert!(
        validate_runtime_dag_qualification_history(
            temp.path(),
            &substituted,
            Some(&binding),
            None,
        )
        .is_err()
    );

    let bytes = norito::to_bytes(&history).expect("encode canonical history");
    assert!(
        decode_canonical_runtime_dag::<RuntimeDagQualificationHistoryV1>(
            &bytes[..bytes.len() - 1],
            "truncated qualification history",
        )
        .is_err()
    );
    let mut trailing = bytes;
    trailing.push(0);
    assert!(
        decode_canonical_runtime_dag::<RuntimeDagQualificationHistoryV1>(
            &trailing,
            "qualification history with trailing bytes",
        )
        .is_err()
    );
}

#[test]
fn filesystem_publisher_recovers_typed_stage_after_ambiguous_intent_cas() {
    let temp = tempdir().expect("tempdir");
    let checkpoint_store = Arc::new(TestRuntimeDagCheckpointStore::default());
    checkpoint_store
        .fail_after_next_intent_cas
        .store(true, Ordering::SeqCst);
    {
        let publisher =
            signed_runtime_publisher_with_store(temp.path(), Arc::clone(&checkpoint_store));
        let (settlement, encoded) = sample_settlement();
        let error = publisher
            .publish_deal_settlement(&settlement, &encoded)
            .expect_err("ambiguous intent CAS response must surface");
        assert!(error.to_string().contains("compare-and-swap failed"));
    }

    assert!(
        checkpoint_store
            .load(GovernanceDagSealedStateSlot::ProducerPublishIntent)
            .expect("load sealed producer intent")
            .is_some(),
        "ambiguous CAS must retain the sealed intent"
    );
    let publisher = signed_runtime_publisher_with_store(temp.path(), Arc::clone(&checkpoint_store));
    assert!(
        checkpoint_store
            .load(GovernanceDagSealedStateSlot::ProducerPublishIntent)
            .expect("reload producer intent")
            .is_none()
    );
    let index = runtime_index(temp.path());
    assert_eq!(
        index.get("block_count").and_then(JsonValue::as_u64),
        Some(1)
    );
    drop(publisher);
}

#[test]
fn filesystem_publisher_replays_typed_transaction_from_sealed_intent() {
    for _replay in 0..1 {
        let temp = tempdir().expect("tempdir");
        let checkpoint_store = Arc::new(TestRuntimeDagCheckpointStore::default());
        let publisher =
            signed_runtime_publisher_with_store(temp.path(), Arc::clone(&checkpoint_store));
        let (first, first_encoded) = sample_settlement();
        publisher
            .publish_deal_settlement(&first, &first_encoded)
            .expect("seed predecessor runtime DAG block");

        let mut successor = first;
        successor.deal_id = [0x42; 32];
        successor.ledger.deal_id = successor.deal_id;
        successor.ledger.snapshot_id = successor
            .ledger
            .derive_snapshot_id()
            .expect("reseal successor ledger snapshot");
        successor.settlement_id = successor
            .derive_settlement_id()
            .expect("reseal successor settlement");
        let successor_encoded = norito::to_bytes(&successor).expect("encode successor settlement");
        checkpoint_store
            .fail_after_next_intent_cas
            .store(true, Ordering::SeqCst);
        publisher
            .publish_deal_settlement(&successor, &successor_encoded)
            .expect_err("retain sealed successor intent before filesystem apply");

        let intent_record = checkpoint_store
            .load(GovernanceDagSealedStateSlot::ProducerPublishIntent)
            .expect("load retained producer intent")
            .expect("producer intent exists");
        let intent: RuntimeDagProducerPublishIntentV1 =
            norito::decode_from_bytes(&intent_record.payload).expect("decode producer intent");
        let staged = load_runtime_dag_producer_staged_transaction(
            temp.path(),
            publisher.root_guard(),
            &intent,
        )
        .expect("load exact staged transaction");
        let signer = publisher
            .runtime_dag_signer
            .as_ref()
            .expect("test publisher signer");
        let store = publisher
            .runtime_dag_checkpoint_store
            .as_ref()
            .expect("test publisher checkpoint store");
        let previous_record = store
            .load(GovernanceDagSealedStateSlot::ProducerCheckpoint)
            .expect("load predecessor checkpoint")
            .expect("predecessor checkpoint exists");
        let previous = decode_runtime_dag_producer_checkpoint_record(
            &previous_record,
            temp.path(),
            signer,
            store,
        )
        .expect("decode predecessor checkpoint");
        validate_runtime_dag_producer_intent_successor(
            temp.path(),
            &publisher.root_guard,
            signer,
            &intent,
            &staged,
            Some(&previous),
        )
        .expect("authenticate successor before applying it");
        drop(publisher);

        let recovered =
            signed_runtime_publisher_with_store(temp.path(), Arc::clone(&checkpoint_store));
        assert!(
            checkpoint_store
                .load(GovernanceDagSealedStateSlot::ProducerPublishIntent)
                .expect("reload producer intent")
                .is_none()
        );
        assert_eq!(
            runtime_index(temp.path())
                .get("block_count")
                .and_then(JsonValue::as_u64),
            Some(2)
        );
        drop(recovered);
    }
}

#[test]
fn filesystem_publisher_clamps_clock_regression_before_sealing_and_recovers() {
    let temp = tempdir().expect("tempdir");
    let checkpoint_store = Arc::new(TestRuntimeDagCheckpointStore::default());
    let publisher = signed_runtime_publisher_with_store(temp.path(), Arc::clone(&checkpoint_store));
    let (first, first_encoded) = sample_settlement();
    publisher
        .publish_deal_settlement(&first, &first_encoded)
        .expect("seed predecessor runtime DAG block");
    let predecessor_timestamp = runtime_index(temp.path())
        .get("head_generated_at")
        .and_then(JsonValue::as_u64)
        .expect("predecessor head timestamp");
    publisher.set_runtime_dag_observed_timestamp_for_test(predecessor_timestamp.saturating_sub(1));

    let mut successor = first;
    successor.deal_id = [0x43; 32];
    successor.ledger.deal_id = successor.deal_id;
    successor.ledger.snapshot_id = successor
        .ledger
        .derive_snapshot_id()
        .expect("reseal successor ledger snapshot");
    successor.settlement_id = successor
        .derive_settlement_id()
        .expect("reseal successor settlement");
    let successor_encoded = norito::to_bytes(&successor).expect("encode successor settlement");
    checkpoint_store
        .fail_after_next_intent_cas
        .store(true, Ordering::SeqCst);
    publisher
        .publish_deal_settlement(&successor, &successor_encoded)
        .expect_err("retain the monotonically timestamped successor intent");
    let intent_record = checkpoint_store
        .load(GovernanceDagSealedStateSlot::ProducerPublishIntent)
        .expect("load retained producer intent")
        .expect("producer intent exists");
    let intent: RuntimeDagProducerPublishIntentV1 =
        norito::decode_from_bytes(&intent_record.payload).expect("decode producer intent");
    let staged =
        load_runtime_dag_producer_staged_transaction(temp.path(), publisher.root_guard(), &intent)
            .expect("load staged clock-regression successor");
    let block: GovernanceDagBlockV1 =
        decode_canonical_runtime_dag(&staged.block_bytes, "clock-regression successor block")
            .expect("decode successor block");
    assert_eq!(block.timestamp, predecessor_timestamp);
    assert_eq!(block.node.timestamp, predecessor_timestamp);
    drop(publisher);

    let recovered = signed_runtime_publisher_with_store(temp.path(), Arc::clone(&checkpoint_store));
    assert!(
        checkpoint_store
            .load(GovernanceDagSealedStateSlot::ProducerPublishIntent)
            .expect("reload producer intent")
            .is_none(),
        "restart must complete the clamped successor rather than wedge on timestamp regression"
    );
    assert_eq!(
        runtime_index(temp.path())
            .get("block_count")
            .and_then(JsonValue::as_u64),
        Some(2)
    );
    drop(recovered);
}
