// Publication persistence, crash recovery, and artifact-boundary regressions.
fn try_publisher(root: &Path) -> io::Result<FilesystemGovernancePublisher> {
    FilesystemGovernancePublisher::try_new(root.to_path_buf())
}

fn empty_publication_root() -> CanonicalTempDir {
    let temp = tempdir().expect("tempdir");
    drop(try_publisher(temp.path()).expect("initialize empty publication authority"));
    temp
}

fn interrupted_fixture(
    payload_kind: &str,
    encoded: &[u8],
    position: usize,
) -> (CanonicalTempDir, PublishIndexEntryForCar) {
    let temp = empty_publication_root();
    let (entry, _) =
        seed_complete_uncommitted_publication_fixture(temp.path(), payload_kind, encoded, position);
    (temp, entry)
}

fn committed_settlement_publisher(root: &Path) -> FilesystemGovernancePublisher {
    let publisher = try_publisher(root).expect("publisher");
    let (settlement, encoded) = sample_settlement();
    publisher
        .publish_deal_settlement(&settlement, &encoded)
        .expect("publish committed settlement");
    publisher
}

fn startup_error(root: &Path, expectation: &str) -> io::Error {
    try_publisher(root).expect_err(expectation)
}

fn json_str<'a>(value: &'a JsonValue, field: &str) -> Option<&'a str> {
    value.get(field).and_then(JsonValue::as_str)
}

fn json_u64(value: &JsonValue, field: &str) -> Option<u64> {
    value.get(field).and_then(JsonValue::as_u64)
}

fn json_object<'a>(value: &'a JsonValue, field: &str) -> Option<&'a JsonMap> {
    value.get(field).and_then(JsonValue::as_object)
}

fn json_array<'a>(value: &'a JsonValue, field: &str) -> Option<&'a Vec<JsonValue>> {
    value.get(field).and_then(JsonValue::as_array)
}

#[test]
fn filesystem_publisher_rejects_malformed_runtime_dag_index_in_committed_state() {
    let temp = tempdir().expect("tempdir");
    let publisher = signed_runtime_publisher(temp.path());
    let (settlement, encoded) = sample_settlement();
    publisher
        .publish_deal_settlement(&settlement, &encoded)
        .expect("seed signed runtime DAG");
    let store = open_runtime_dag_committed_store_v1(temp.path(), publisher.root_guard())
        .expect("open committed runtime DAG state");
    let (mut committed, snapshot) =
        load_runtime_dag_committed_state_v1(&store).expect("load committed runtime DAG state");
    let mut index: JsonValue = json::from_slice(
        committed
            .index_bytes
            .as_deref()
            .expect("committed runtime index bytes"),
    )
    .expect("decode committed runtime index");
    index.as_object_mut().expect("runtime index object").insert(
        "schema".to_owned(),
        JsonValue::from("sorafs.governance_dag.wrong"),
    );
    committed.index_bytes = Some(
        json::to_json_pretty(&index)
            .expect("encode malformed runtime index")
            .into_bytes(),
    );
    let bytes =
        encode_governance_two_slot_value_v1(&committed, "malformed committed runtime DAG state")
            .expect("encode malformed committed state");
    compare_and_swap_governance_two_slot_store_v1(
        &store,
        &snapshot,
        &bytes,
        "malformed committed runtime DAG state",
    )
    .expect("commit malformed semantic fixture");
    drop(store);
    let err = validate_existing_runtime_dag_root(
        temp.path(),
        publisher
            .runtime_dag_signer
            .as_ref()
            .expect("signed publisher"),
        publisher
            .runtime_dag_checkpoint_store
            .as_ref()
            .expect("signed publisher store"),
    )
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
    let publication_snapshot = read_publication_snapshot_fixture(temp.path());
    let publication_identity = publication_snapshot.store_identity();
    let publication_bytes = publication_snapshot.canonical_bytes().to_vec();
    let publication: JsonValue = norito::json::from_slice(&publication_bytes)
        .expect("decode authoritative publication snapshot");
    assert_eq!(
        json_str(&publication, "schema"),
        Some(GOVERNANCE_PUBLICATION_STATE_SCHEMA)
    );
    assert_eq!(json_u64(&publication, "generation"), Some(1));
    let index = publication
        .get("publish_index")
        .cloned()
        .expect("nested publish index");
    assert_eq!(
        json_str(&index, "schema"),
        Some(GOVERNANCE_PUBLISH_INDEX_SCHEMA)
    );
    assert_eq!(json_str(&index, "root"), Some(GOVERNANCE_DAG_LOGICAL_ROOT));
    assert_eq!(json_u64(&index, "entry_count"), Some(1));
    assert_eq!(
        json_object(&index, "payload_kind_counts")
            .and_then(|counts| counts.get("deal_settlement"))
            .and_then(JsonValue::as_u64),
        Some(1)
    );
    let digest_hex = blake3::hash(&encoded).to_hex().to_string();
    let digest_positions = json_object(&index, "by_encoded_blake3")
        .and_then(|map| map.get(digest_hex.as_str()))
        .and_then(JsonValue::as_array)
        .expect("digest lookup");
    assert_eq!(digest_positions.len(), 1);
    assert_eq!(digest_positions[0].as_u64(), Some(0));
    let kind_positions = json_object(&index, "by_payload_kind")
        .and_then(|map| map.get("deal_settlement"))
        .and_then(JsonValue::as_array)
        .expect("kind lookup");
    assert_eq!(kind_positions[0].as_u64(), Some(0));
    let entry = json_array(&index, "entries")
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
        json_str(&queue, "schema"),
        Some(GOVERNANCE_CAR_QUEUE_SCHEMA)
    );
    assert_eq!(json_str(&queue, "root"), Some(GOVERNANCE_DAG_LOGICAL_ROOT));
    assert_eq!(json_u64(&queue, "segment_count"), Some(1));
    assert_eq!(json_u64(&queue, "assembled_count"), Some(1));
    let segment = json_array(&queue, "segments")
        .and_then(|segments| segments.first())
        .expect("first CAR segment");
    assert_eq!(
        json_str(segment, "schema"),
        Some(GOVERNANCE_CAR_SEGMENT_SCHEMA)
    );
    assert_eq!(json_str(segment, "status"), Some("assembled"));
    assert_eq!(json_u64(segment, "source_publish_index_position"), Some(0));
    assert_eq!(
        json_str(segment, "encoded_blake3"),
        Some(digest_hex.as_str())
    );
    let car_path = resolve_index_path(
        temp.path(),
        json_str(segment, "car_path").expect("car path"),
    )
    .expect("resolve car path");
    let car_bytes = fs::read(&car_path).expect("read CAR segment");
    let car_archive_digest_hex = blake3::hash(&car_bytes).to_hex().to_string();
    assert_eq!(json_u64(segment, "car_size"), Some(car_bytes.len() as u64));
    assert_eq!(
        json_str(segment, "car_archive_blake3"),
        Some(car_archive_digest_hex.as_str())
    );
    let archive_positions = json_object(&queue, "by_car_archive_blake3")
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
        json_str(segment, "plan_path").expect("plan path"),
    )
    .expect("resolve plan path");
    let plan_bytes = fs::read(&plan_path).expect("read CAR plan");
    let plan: JsonValue = norito::json::from_slice(&plan_bytes).expect("plan json");
    assert_eq!(json_str(&plan, "schema"), Some(GOVERNANCE_CAR_PLAN_SCHEMA));
    assert_eq!(json_u64(&plan, "source_publish_index_position"), Some(0));
    assert_eq!(json_array(&plan, "files").map(Vec::len), Some(4));
    assert!(
        json_array(&plan, "chunks").is_some_and(|chunks| !chunks.is_empty()),
        "CAR plan should expose deterministic chunks"
    );
    let manifest_path = resolve_index_path(
        temp.path(),
        json_str(segment, "manifest_path").expect("manifest path"),
    )
    .expect("resolve segment manifest path");
    let manifest_bytes = fs::read(&manifest_path).expect("read segment manifest");
    assert!(manifest_bytes.len() <= GOVERNANCE_CAR_SEGMENT_MANIFEST_MAX_BYTES_V1);
    let manifest: JsonValue =
        norito::json::from_slice(&manifest_bytes).expect("segment manifest json");
    assert_eq!(
        json_str(&manifest, "schema"),
        Some(GOVERNANCE_CAR_SEGMENT_SCHEMA)
    );
    publisher
        .publish_deal_settlement(&settlement, &encoded)
        .expect("exact duplicate publication is a no-op");
    let duplicate_snapshot = read_publication_snapshot_fixture(temp.path());
    assert_eq!(
        duplicate_snapshot.store_identity(),
        publication_identity,
        "an exact duplicate must not advance the typed authority identity"
    );
    assert_eq!(
        duplicate_snapshot.canonical_bytes(),
        publication_bytes.as_slice(),
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
    let rejected_snapshot = read_publication_snapshot_fixture(temp.path());
    assert_eq!(
        rejected_snapshot.store_identity(),
        publication_identity,
        "a rejected duplicate must preserve the typed authority identity"
    );
    assert_eq!(
        rejected_snapshot.canonical_bytes(),
        publication_bytes.as_slice(),
        "a rejected duplicate must leave the authority envelope unchanged"
    );
    assert_eq!(
        fs::read(&car_path).expect("read rejected substituted CAR"),
        b"substituted archive",
        "the publisher must not conceal immutable-artifact substitution by overwriting it"
    );
    let publication = read_publication_state_fixture(temp.path());
    assert_eq!(json_u64(&publication, "generation"), Some(1));
    let index = publication
        .get("publish_index")
        .expect("republished nested index");
    assert_eq!(
        json_u64(index, "entry_count"),
        Some(1),
        "duplicate attempts must not duplicate the index entry"
    );
    let queue = publication
        .get("car_queue")
        .expect("republished nested queue");
    assert_eq!(
        json_u64(queue, "segment_count"),
        Some(1),
        "duplicate attempts must not duplicate the CAR queue segment"
    );
}
#[test]
fn filesystem_publisher_settlement_json_preserves_exact_wide_quantities() {
    let temp = tempdir().expect("tempdir");
    let publisher = try_publisher(temp.path()).expect("publisher");
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
    let object = json_object(&value, "settlement").expect("settlement object");
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
    let publisher = try_publisher(temp.path()).expect("publisher");
    let (settlement, encoded) = sample_settlement();
    let legacy_queue = temp.path().join(GOVERNANCE_CAR_QUEUE_FILE);
    let legacy_body: &[u8] = br#"{"schema":"wrong","segments":[]}"#;
    fs::write(&legacy_queue, legacy_body).expect("write malformed queue");
    let err = publisher
        .publish_deal_settlement(&settlement, &encoded)
        .expect_err("legacy CAR queue authority must fail closed");
    assert!(
        err.to_string()
            .contains("legacy governance publication authority"),
        "unexpected error: {err}"
    );
    assert_eq!(
        fs::read(&legacy_queue).expect("read retained legacy CAR authority"),
        legacy_body,
        "online rejection must not alter the legacy authority"
    );
    assert!(
        !temp
            .path()
            .join(GOVERNANCE_PUBLICATION_SOURCES_DIR)
            .exists(),
        "legacy authority rejection must precede immutable source writes"
    );
    assert!(
        !temp.path().join(GOVERNANCE_CAR_SEGMENTS_DIR).exists(),
        "legacy authority rejection must precede immutable CAR writes"
    );
}
#[test]
fn filesystem_publisher_rejects_legacy_flat_publication_authority_before_artifact_writes() {
    let temp = tempdir().expect("tempdir");
    fs::write(
        temp.path().join(GOVERNANCE_PUBLICATION_STATE_FILE),
        br#"{"schema":"substituted"}"#,
    )
    .expect("write malformed authoritative publication state");
    let error = startup_error(
        temp.path(),
        "legacy flat authority must reject publisher startup",
    );
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert!(
        error
            .to_string()
            .contains("legacy governance publication authority")
    );
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
    let temp = empty_publication_root();
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
    }
    assert!(
        temp.path()
            .join(GOVERNANCE_PUBLICATION_SOURCES_DIR)
            .exists()
    );
    assert!(temp.path().join(GOVERNANCE_CAR_SEGMENTS_DIR).exists());
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let error = startup_error(
            temp.path(),
            "Unix recovery must isolate interrupted artifacts before startup",
        );
        assert!(
            error
                .to_string()
                .contains(GOVERNANCE_PUBLICATION_RECOVERY_QUARANTINE_DIR),
            "unexpected quarantine error: {error}"
        );
        let quarantine = recovery_quarantine_path(temp.path());
        let isolated = fs::read_dir(&quarantine)
            .expect("read bounded recovery quarantine")
            .map(|entry| entry.expect("quarantine entry").file_name())
            .collect::<BTreeSet<_>>();
        let expected = [
            "car-file-00",
            "car-file-01",
            "car-file-02",
            "car-file-03",
            "car-file-04",
            "car-file-05",
            "car-root",
            "source-file-00",
            "source-file-01",
            "source-file-02",
            "source-file-03",
            "source-kind",
            "source-pair",
            "source-root",
        ]
        .map(OsString::from)
        .into_iter()
        .collect::<BTreeSet<_>>();
        assert_eq!(isolated, expected, "quarantine slots are deterministic");
        clear_recovery_quarantine_offline(temp.path());
        drop(
            try_publisher(temp.path()).expect("startup succeeds after offline quarantine cleanup"),
        );
    }
    #[cfg(windows)]
    drop(
        try_publisher(temp.path())
            .expect("Windows exact-handle cleanup reconciles the publication"),
    );
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
}
#[test]
fn filesystem_publisher_reclaims_only_the_exact_next_car_atomic_temp() {
    let temp = empty_publication_root();
    let orphan = write_car_segment_source_fixture(temp.path(), b"orphan-publication");
    let car_base = temp
        .path()
        .join(governance_car_segment_relative_base(&orphan).expect("CAR base"));
    let car_target = car_base.with_extension("car");
    let car_directory = car_target.parent().expect("CAR directory");
    fs::create_dir_all(car_directory).expect("create interrupted CAR directory");
    let car_target_name = car_target
        .file_name()
        .and_then(OsStr::to_str)
        .expect("canonical CAR target name");
    fs::write(
        car_directory.join(format!(".{car_target_name}.tmp-42000-1")),
        b"interrupted CAR temp",
    )
    .expect("seed exact next CAR temp");
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let error = startup_error(
            temp.path(),
            "Unix recovery must isolate the exact interrupted temp",
        );
        assert!(
            error
                .to_string()
                .contains(GOVERNANCE_PUBLICATION_RECOVERY_QUARANTINE_DIR),
            "unexpected quarantine error: {error}"
        );
        clear_recovery_quarantine_offline(temp.path());
        drop(
            try_publisher(temp.path()).expect("startup succeeds after offline quarantine cleanup"),
        );
    }
    #[cfg(windows)]
    drop(
        try_publisher(temp.path())
            .expect("Windows exact-handle cleanup reconciles the interrupted temp"),
    );
    assert!(
        !temp
            .path()
            .join(GOVERNANCE_PUBLICATION_SOURCES_DIR)
            .exists()
    );
    assert!(!temp.path().join(GOVERNANCE_CAR_SEGMENTS_DIR).exists());
}
#[test]
fn filesystem_publisher_verifies_every_committed_role_before_orphan_cleanup() {
    #[derive(Clone, Copy, Debug)]
    enum Mutation {
        Missing,
        Corrupt,
    }
    for mutation in [Mutation::Missing, Mutation::Corrupt] {
        for role_index in 0..10 {
            let temp = tempdir().expect("tempdir");
            let publisher = committed_settlement_publisher(temp.path());
            let state = read_publication_state_fixture(temp.path());
            let committed = committed_publication_artifact_paths(
                temp.path(),
                state.as_object().expect("publication state object"),
            );
            drop(publisher);
            let (_, orphan_snapshots) = seed_complete_uncommitted_publication_fixture(
                temp.path(),
                "interrupted_test_payload",
                b"interrupted-publication",
                1,
            );
            let (role, committed_path) = committed
                .into_iter()
                .nth(role_index)
                .expect("committed role index");
            match mutation {
                Mutation::Missing => {
                    fs::remove_file(&committed_path)
                        .expect("remove one committed publication role");
                }
                Mutation::Corrupt => {
                    fs::write(&committed_path, b"corrupt committed publication artifact")
                        .expect("corrupt one committed publication role");
                }
            }
            let error = startup_error(
                temp.path(),
                "startup must reject a missing or corrupt committed publication role",
            );
            assert_eq!(
                error.kind(),
                io::ErrorKind::InvalidData,
                "unexpected error kind for {mutation:?} {role}: {error}"
            );
            for (orphan_path, expected) in &orphan_snapshots {
                let actual = fs::read(orphan_path).unwrap_or_else(|error| {
                    panic!(
                        "{mutation:?} {role} deleted orphan `{}` before failing: {error}",
                        orphan_path.display()
                    )
                });
                assert_eq!(
                    actual.as_slice(),
                    expected.as_slice(),
                    "{mutation:?} {role} changed orphan `{}` before failing",
                    orphan_path.display()
                );
            }
        }
    }
}
#[test]
fn filesystem_publisher_rejects_multiple_interrupted_source_pairs_without_cleanup() {
    let temp = empty_publication_root();
    let first = write_car_segment_source_fixture_for_kind(
        temp.path(),
        "interrupted_alpha",
        b"interrupted-alpha",
    );
    let second = write_car_segment_source_fixture_for_kind(
        temp.path(),
        "interrupted_beta",
        b"interrupted-beta",
    );
    let snapshots = [first, second]
        .into_iter()
        .flat_map(|entry| {
            let encoded = temp.path().join(entry.encoded_path);
            let json = temp.path().join(entry.json_path);
            [
                encoded.clone(),
                digest_sidecar_path_for(&encoded),
                json.clone(),
                digest_sidecar_path_for(&json),
            ]
        })
        .map(|path| {
            let bytes = fs::read(&path).expect("snapshot interrupted source role");
            (path, bytes)
        })
        .collect::<Vec<_>>();
    let error = startup_error(
        temp.path(),
        "multiple interrupted source identities must fail closed",
    );
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    for (path, expected) in snapshots {
        assert_eq!(
            fs::read(&path).expect("multiple-source rejection preserves every artifact"),
            expected,
            "multiple-source rejection changed `{}`",
            path.display()
        );
    }
}
#[test]
fn filesystem_publisher_rejects_split_interrupted_car_bases_without_cleanup() {
    let (temp, entry) = interrupted_fixture("interrupted_split_car", b"interrupted-split-car", 0);
    let original_base = temp
        .path()
        .join(governance_car_segment_relative_base(&entry).expect("derive original CAR base"));
    let pair_id = original_base
        .file_name()
        .and_then(OsStr::to_str)
        .and_then(|base| base.split_once('_'))
        .map(|(_, pair_id)| pair_id)
        .expect("fixture CAR pair identity");
    let alternate_base = temp
        .path()
        .join(GOVERNANCE_CAR_SEGMENTS_DIR)
        .join(format!("{:020}_{pair_id}", 1));
    for suffix in ["json", "json.blake3"] {
        let source = original_base.with_extension(suffix);
        let target = alternate_base.with_extension(suffix);
        fs::rename(&source, &target).expect("split CAR role across another base");
    }
    let snapshots = fs::read_dir(temp.path().join(GOVERNANCE_CAR_SEGMENTS_DIR))
        .expect("read split CAR directory")
        .map(|entry| {
            let path = entry.expect("split CAR entry").path();
            let bytes = fs::read(&path).expect("snapshot split CAR role");
            (path, bytes)
        })
        .chain(
            publication_artifact_paths_for_fixture(temp.path(), &entry)
                .into_iter()
                .take(GOVERNANCE_PUBLICATION_SOURCE_PAIR_ARTIFACT_COUNT)
                .map(|path| {
                    let bytes = fs::read(&path).expect("snapshot split source role");
                    (path, bytes)
                }),
        )
        .collect::<Vec<_>>();
    let error = startup_error(temp.path(), "CAR roles split across bases must fail closed");
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert!(
        error.to_string().contains("more than one artifact base"),
        "unexpected error: {error}"
    );
    for (path, expected) in snapshots {
        assert_eq!(
            fs::read(&path).expect("split-base rejection preserves every artifact"),
            expected,
            "split-base rejection changed `{}`",
            path.display()
        );
    }
}
#[test]
fn filesystem_publisher_rejects_non_next_or_uncorrelated_interrupted_car_without_cleanup() {
    for (case, replacement_position, replacement_pair_id) in [
        ("non-next", 1_usize, None),
        ("uncorrelated", 0_usize, Some("ab".repeat(32))),
    ] {
        let (temp, entry) = interrupted_fixture(
            "interrupted_identity_check",
            b"interrupted-identity-check",
            0,
        );
        let original_base = temp
            .path()
            .join(governance_car_segment_relative_base(&entry).expect("derive original CAR base"));
        let original_pair_id = original_base
            .file_name()
            .and_then(OsStr::to_str)
            .and_then(|base| base.split_once('_'))
            .map(|(_, pair_id)| pair_id)
            .expect("fixture CAR pair identity");
        let replacement_pair_id = replacement_pair_id.as_deref().unwrap_or(original_pair_id);
        let replacement_base = temp
            .path()
            .join(GOVERNANCE_CAR_SEGMENTS_DIR)
            .join(format!("{replacement_position:020}_{replacement_pair_id}"));
        for suffix in [
            "car",
            "car.blake3",
            "plan.json",
            "plan.json.blake3",
            "json",
            "json.blake3",
        ] {
            fs::rename(
                original_base.with_extension(suffix),
                replacement_base.with_extension(suffix),
            )
            .expect("move CAR role to a single invalid interrupted base");
        }
        let snapshots = fs::read_dir(temp.path().join(GOVERNANCE_CAR_SEGMENTS_DIR))
            .expect("read invalid CAR directory")
            .map(|entry| {
                let path = entry.expect("invalid CAR entry").path();
                let bytes = fs::read(&path).expect("snapshot invalid CAR role");
                (path, bytes)
            })
            .chain(
                publication_artifact_paths_for_fixture(temp.path(), &entry)
                    .into_iter()
                    .take(GOVERNANCE_PUBLICATION_SOURCE_PAIR_ARTIFACT_COUNT)
                    .map(|path| {
                        let bytes = fs::read(&path).expect("snapshot source role");
                        (path, bytes)
                    }),
            )
            .collect::<Vec<_>>();
        let error = startup_error(
            temp.path(),
            "invalid interrupted CAR identity must fail closed",
        );
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        let message = error.to_string();
        assert!(
            message.contains("exact expected next position")
                || message.contains("source and CAR identities diverge"),
            "unexpected {case} error: {error}"
        );
        for (path, expected) in snapshots {
            assert_eq!(
                fs::read(&path).expect("identity rejection preserves every artifact"),
                expected,
                "{case} rejection changed `{}`",
                path.display()
            );
        }
    }
}
#[test]
fn filesystem_publisher_cleanup_is_restart_safe_after_every_rollback_step() {
    const CLEANUP_STEPS: usize = GOVERNANCE_PUBLICATION_CAR_ARTIFACT_COUNT
        + 1
        + GOVERNANCE_PUBLICATION_SOURCE_PAIR_ARTIFACT_COUNT
        + 3;
    for interrupted_after in 1..=CLEANUP_STEPS {
        let (temp, _) = interrupted_fixture(
            "interrupted_rollback_boundary",
            b"interrupted-rollback-boundary",
            0,
        );
        let root_guard = GovernanceFilesystemRootGuard::capture_writer(temp.path())
            .expect("retain rollback fixture root");
        let state = read_publication_state_fixture(temp.path());
        let state = state.as_object().expect("publication state object");
        let inventory = governance_publication_artifact_inventory(state)
            .expect("derive rollback fixture inventory");
        let (mut cleanup_plan, interrupted_identity) =
            plan_governance_publication_source_artifacts(&root_guard, &inventory)
                .expect("plan interrupted source rollback");
        plan_governance_publication_car_artifacts(
            &root_guard,
            &inventory,
            interrupted_identity.as_ref(),
            &mut cleanup_plan,
        )
        .expect("plan interrupted CAR rollback");
        verify_governance_publication_artifact_integrity(temp.path(), &root_guard, state)
            .expect("verify empty committed authority");
        let observed_step = std::cell::Cell::new(0_usize);
        let error =
            apply_governance_publication_cleanup_plan_with(&root_guard, cleanup_plan, |step| {
                observed_step.set(step);
                if step == interrupted_after {
                    Err(GovernancePublishError::other(
                        "injected cleanup interruption",
                    ))
                } else {
                    Ok(())
                }
            })
            .expect_err("injected rollback interruption must stop cleanup");
        assert!(error.to_string().contains("injected cleanup interruption"));
        assert_eq!(observed_step.get(), interrupted_after);
        drop(root_guard);
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        let publisher = {
            let quarantine = recovery_quarantine_path(temp.path());
            let before_restart = fs::read_dir(&quarantine)
                .expect("read interrupted recovery quarantine")
                .count();
            assert_eq!(before_restart, interrupted_after);
            let restart_error = startup_error(
                temp.path(),
                "restart must stop at a preserved recovery quarantine",
            );
            assert!(
                restart_error
                    .to_string()
                    .contains(GOVERNANCE_PUBLICATION_RECOVERY_QUARANTINE_DIR),
                "unexpected restart error: {restart_error}"
            );
            assert_eq!(
                fs::read_dir(&quarantine)
                    .expect("reread preserved recovery quarantine")
                    .count(),
                before_restart,
                "restart must not mutate a preserved quarantine"
            );
            clear_recovery_quarantine_offline(temp.path());
            finish_recovery_after_offline_quarantine_cleanup(temp.path())
        };
        #[cfg(windows)]
        let publisher = try_publisher(temp.path()).unwrap_or_else(|error| {
            panic!("restart after cleanup step {interrupted_after}/{CLEANUP_STEPS} failed: {error}")
        });
        assert!(
            !temp
                .path()
                .join(GOVERNANCE_PUBLICATION_SOURCES_DIR)
                .exists(),
            "source residue remained after restarting cleanup step {interrupted_after}"
        );
        assert!(
            !temp.path().join(GOVERNANCE_CAR_SEGMENTS_DIR).exists(),
            "CAR residue remained after restarting cleanup step {interrupted_after}"
        );
        drop(publisher);
    }
}
#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn filesystem_publisher_quarantines_same_inode_byte_changes_after_cleanup_planning() {
    let (temp, entry) =
        interrupted_fixture("interrupted_byte_change", b"interrupted-byte-change", 0);
    let root_guard = GovernanceFilesystemRootGuard::capture_writer(temp.path())
        .expect("retain byte-change fixture root");
    let state = read_publication_state_fixture(temp.path());
    let state = state.as_object().expect("publication state object");
    let inventory = governance_publication_artifact_inventory(state)
        .expect("derive byte-change fixture inventory");
    let (mut cleanup_plan, interrupted_identity) =
        plan_governance_publication_source_artifacts(&root_guard, &inventory)
            .expect("plan interrupted source rollback");
    plan_governance_publication_car_artifacts(
        &root_guard,
        &inventory,
        interrupted_identity.as_ref(),
        &mut cleanup_plan,
    )
    .expect("plan interrupted CAR rollback");
    verify_governance_publication_artifact_integrity(temp.path(), &root_guard, state)
        .expect("verify empty committed authority");
    let car_roles = publication_artifact_paths_for_fixture(temp.path(), &entry)
        .into_iter()
        .skip(GOVERNANCE_PUBLICATION_SOURCE_PAIR_ARTIFACT_COUNT)
        .collect::<Vec<_>>();
    let first_rollback_role = car_roles
        .last()
        .expect("complete CAR fixture has a final rollback role");
    let original = fs::read(first_rollback_role).expect("read planned CAR role");
    let substituted = vec![b'x'; original.len()];
    fs::write(first_rollback_role, &substituted)
        .expect("change planned CAR bytes without replacing its inode");
    let error = apply_governance_publication_cleanup_plan(&root_guard, cleanup_plan)
        .expect_err("post-plan byte change must stop recovery after isolation");
    assert!(
        error.to_string().contains("changed after exact comparison"),
        "unexpected byte-comparison error: {error}"
    );
    assert!(
        !first_rollback_role.exists(),
        "the changed live binding must be isolated without unlinking"
    );
    assert_eq!(
        fs::read(recovery_quarantine_path(temp.path()).join("car-file-05"))
            .expect("read preserved changed CAR role"),
        substituted,
        "the changed same-inode bytes must remain available for offline inspection"
    );
}
#[test]
fn filesystem_publisher_rolls_back_the_next_atomic_temp_before_durable_roles() {
    let (temp, entry) =
        interrupted_fixture("interrupted_temp_rollback", b"interrupted-temp-rollback", 0);
    let car_roles = publication_artifact_paths_for_fixture(temp.path(), &entry)
        .into_iter()
        .skip(GOVERNANCE_PUBLICATION_SOURCE_PAIR_ARTIFACT_COUNT)
        .collect::<Vec<_>>();
    for path in car_roles.iter().skip(1) {
        fs::remove_file(path).expect("truncate CAR prefix after its archive");
    }
    let next_target = &car_roles[1];
    let next_name = next_target
        .file_name()
        .and_then(OsStr::to_str)
        .expect("next CAR role name");
    let next_temp = next_target
        .parent()
        .expect("CAR role parent")
        .join(format!(".{next_name}.tmp-42000-1"));
    fs::write(&next_temp, b"partially-written-sidecar").expect("seed next atomic temporary");
    let root_guard = GovernanceFilesystemRootGuard::capture_writer(temp.path())
        .expect("retain temporary rollback root");
    let state = read_publication_state_fixture(temp.path());
    let state = state.as_object().expect("publication state object");
    let inventory = governance_publication_artifact_inventory(state)
        .expect("derive temporary rollback inventory");
    let (mut cleanup_plan, interrupted_identity) =
        plan_governance_publication_source_artifacts(&root_guard, &inventory)
            .expect("plan temporary source rollback");
    plan_governance_publication_car_artifacts(
        &root_guard,
        &inventory,
        interrupted_identity.as_ref(),
        &mut cleanup_plan,
    )
    .expect("plan temporary CAR rollback");
    verify_governance_publication_artifact_integrity(temp.path(), &root_guard, state)
        .expect("verify empty committed authority");
    apply_governance_publication_cleanup_plan_with(&root_guard, cleanup_plan, |step| {
        if step == 1 {
            Err(GovernancePublishError::other(
                "injected post-temporary interruption",
            ))
        } else {
            Ok(())
        }
    })
    .expect_err("cleanup must stop immediately after removing the next temporary");
    assert!(
        !next_temp.exists(),
        "the next temporary must leave the live namespace first"
    );
    assert!(
        car_roles[0].exists(),
        "the durable CAR prefix must remain after the first rollback step"
    );
    drop(root_guard);
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    let publisher = {
        let quarantine = recovery_quarantine_path(temp.path());
        assert_eq!(
            fs::read_dir(&quarantine)
                .expect("read temporary recovery quarantine")
                .count(),
            1,
            "the exact next temporary is the first isolated slot"
        );
        let restart_error = startup_error(
            temp.path(),
            "restart must require offline cleanup of the isolated temp",
        );
        assert!(
            restart_error
                .to_string()
                .contains(GOVERNANCE_PUBLICATION_RECOVERY_QUARANTINE_DIR),
            "unexpected restart error: {restart_error}"
        );
        clear_recovery_quarantine_offline(temp.path());
        finish_recovery_after_offline_quarantine_cleanup(temp.path())
    };
    #[cfg(windows)]
    let publisher =
        try_publisher(temp.path()).expect("restart accepts the preserved durable CAR prefix");
    assert!(!temp.path().join(GOVERNANCE_CAR_SEGMENTS_DIR).exists());
    assert!(
        !temp
            .path()
            .join(GOVERNANCE_PUBLICATION_SOURCES_DIR)
            .exists()
    );
    drop(publisher);
}
#[test]
fn filesystem_publisher_accepts_one_empty_interrupted_kind_and_rejects_two() {
    let accepted = tempdir().expect("tempdir");
    drop(try_publisher(accepted.path()).expect("initialize empty publication authority"));
    fs::create_dir_all(
        accepted
            .path()
            .join(GOVERNANCE_PUBLICATION_SOURCES_DIR)
            .join("interrupted_empty_kind"),
    )
    .expect("seed one durably created empty source kind");
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let error = startup_error(
            accepted.path(),
            "one legitimate empty prefix must be isolated on Unix",
        );
        assert!(
            error
                .to_string()
                .contains(GOVERNANCE_PUBLICATION_RECOVERY_QUARANTINE_DIR),
            "unexpected quarantine error: {error}"
        );
        clear_recovery_quarantine_offline(accepted.path());
        drop(
            try_publisher(accepted.path())
                .expect("startup succeeds after offline quarantine cleanup"),
        );
    }
    #[cfg(windows)]
    drop(
        try_publisher(accepted.path())
            .expect("one empty source-kind prefix is a legitimate interrupted write"),
    );
    assert!(
        !accepted
            .path()
            .join(GOVERNANCE_PUBLICATION_SOURCES_DIR)
            .exists()
    );
    let rejected = tempdir().expect("tempdir");
    drop(try_publisher(rejected.path()).expect("initialize second empty publication authority"));
    let source_root = rejected.path().join(GOVERNANCE_PUBLICATION_SOURCES_DIR);
    for kind in ["interrupted_empty_alpha", "interrupted_empty_beta"] {
        fs::create_dir_all(source_root.join(kind)).expect("seed excess empty source kind");
    }
    let error = startup_error(
        rejected.path(),
        "more than one empty source-kind prefix must fail closed",
    );
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    for kind in ["interrupted_empty_alpha", "interrupted_empty_beta"] {
        assert!(
            source_root.join(kind).is_dir(),
            "excess-prefix rejection removed `{kind}`"
        );
    }
}
#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn filesystem_publisher_rejects_empty_recovery_quarantine_until_offline_cleanup() {
    let temp = empty_publication_root();
    let quarantine = recovery_quarantine_path(temp.path());
    let root_guard = GovernanceFilesystemRootGuard::capture_writer(temp.path())
        .expect("retain empty-quarantine fixture root");
    drop(
        prepare_governance_publication_recovery_quarantine(&root_guard)
            .expect("simulate a crash after durable quarantine creation"),
    );
    drop(root_guard);
    let error = startup_error(
        temp.path(),
        "an empty retained quarantine must still block restart",
    );
    assert!(
        error
            .to_string()
            .contains(GOVERNANCE_PUBLICATION_RECOVERY_QUARANTINE_DIR),
        "unexpected empty-quarantine error: {error}"
    );
    assert_eq!(
        fs::read_dir(&quarantine)
            .expect("reread empty recovery quarantine")
            .count(),
        0,
        "restart must preserve an empty quarantine for explicit offline cleanup"
    );
    clear_recovery_quarantine_offline(temp.path());
    drop(
        try_publisher(temp.path())
            .expect("restart succeeds after removing the empty quarantine offline"),
    );
}
#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn filesystem_publisher_rejects_saturated_recovery_quarantine_without_mutation() {
    let temp = empty_publication_root();
    let quarantine = recovery_quarantine_path(temp.path());
    let root_guard = GovernanceFilesystemRootGuard::capture_writer(temp.path())
        .expect("retain saturated-quarantine fixture root");
    drop(
        prepare_governance_publication_recovery_quarantine(&root_guard)
            .expect("create durable saturated-quarantine fixture"),
    );
    drop(root_guard);
    for position in 0..=GOVERNANCE_PUBLICATION_RECOVERY_QUARANTINE_ENTRY_HARD_CAP {
        fs::write(
            quarantine.join(format!("preserved-{position:02}")),
            position.to_le_bytes(),
        )
        .expect("seed preserved quarantine entry");
    }
    let error = startup_error(
        temp.path(),
        "a saturated recovery quarantine must block startup",
    );
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert!(
        error.to_string().contains("hard cap")
            && error
                .to_string()
                .contains(GOVERNANCE_PUBLICATION_RECOVERY_QUARANTINE_DIR),
        "unexpected saturation error: {error}"
    );
    assert_eq!(
        fs::read_dir(&quarantine)
            .expect("reread saturated quarantine")
            .count(),
        GOVERNANCE_PUBLICATION_RECOVERY_QUARANTINE_ENTRY_HARD_CAP + 1,
        "startup must not mutate a saturated quarantine"
    );
}
#[test]
fn filesystem_publisher_rejects_foreign_car_bytes_at_the_expected_base_without_cleanup() {
    let donor = tempdir().expect("donor tempdir");
    let (donor_entry, _) = seed_complete_uncommitted_publication_fixture(
        donor.path(),
        "foreign_interrupted_payload",
        b"foreign-interrupted-publication",
        0,
    );
    let donor_roles = publication_artifact_paths_for_fixture(donor.path(), &donor_entry)
        .into_iter()
        .skip(GOVERNANCE_PUBLICATION_SOURCE_PAIR_ARTIFACT_COUNT)
        .map(|path| fs::read(path).expect("read foreign CAR role"))
        .collect::<Vec<_>>();
    assert_eq!(donor_roles.len(), GOVERNANCE_PUBLICATION_CAR_ARTIFACT_COUNT);
    for substituted_role in 0..GOVERNANCE_PUBLICATION_CAR_ARTIFACT_COUNT {
        let (temp, entry) = interrupted_fixture(
            "expected_interrupted_payload",
            b"expected-interrupted-publication",
            0,
        );
        let target_roles = publication_artifact_paths_for_fixture(temp.path(), &entry)
            .into_iter()
            .skip(GOVERNANCE_PUBLICATION_SOURCE_PAIR_ARTIFACT_COUNT)
            .collect::<Vec<_>>();
        assert_ne!(
            fs::read(&target_roles[substituted_role]).expect("read expected CAR role"),
            donor_roles[substituted_role],
            "foreign role fixture unexpectedly matches role {substituted_role}"
        );
        fs::write(
            &target_roles[substituted_role],
            &donor_roles[substituted_role],
        )
        .expect("substitute foreign bytes at expected CAR base");
        let snapshots = publication_artifact_paths_for_fixture(temp.path(), &entry)
            .into_iter()
            .map(|path| {
                let bytes = fs::read(&path).expect("snapshot substituted publication role");
                (path, bytes)
            })
            .collect::<Vec<_>>();
        let error = startup_error(
            temp.path(),
            "foreign CAR bytes at the expected base must fail closed",
        );
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(
            error
                .to_string()
                .contains("diverges from its canonical source projection"),
            "unexpected role {substituted_role} error: {error}"
        );
        for (path, expected) in snapshots {
            assert_eq!(
                fs::read(&path).expect("content-correlation rejection preserves role"),
                expected,
                "content-correlation rejection changed `{}` for role {substituted_role}",
                path.display()
            );
        }
    }
}
#[cfg(any(target_os = "linux", target_os = "macos", windows))]
#[test]
fn governance_atomic_writes_reconcile_stale_names_before_mutation() {
    let temp = tempdir().expect("tempdir");
    let root_guard = GovernanceFilesystemRootGuard::capture_writer(temp.path())
        .expect("retain atomic-write fixture root");
    let replace_target = temp.path().join("replace-state");
    let replace_stale = temp.path().join(".replace-state.tmp-42000-1");
    fs::write(&replace_stale, b"older failed write").expect("seed replacement stale temp");
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let error = write_rooted_atomic(&root_guard, &replace_target, b"current write")
            .expect_err("Unix must quarantine a stale replacement before writing");
        assert!(
            error
                .to_string()
                .contains(GOVERNANCE_PUBLICATION_RECOVERY_QUARANTINE_DIR),
            "unexpected replacement quarantine error: {error}"
        );
        assert!(!replace_target.exists());
        assert!(!replace_stale.exists());
        assert_eq!(
            fs::read(recovery_quarantine_path(temp.path()).join("mutable-state-recovery-000000"))
                .expect("read isolated replacement temp"),
            b"older failed write"
        );
        clear_recovery_quarantine_offline(temp.path());
    }
    #[cfg(windows)]
    {
        write_rooted_atomic(&root_guard, &replace_target, b"current write")
            .expect("Windows removes the exact opened stale temp before writing");
        assert_eq!(
            fs::read(&replace_target).expect("read replacement"),
            b"current write"
        );
        assert!(!replace_stale.exists());
    }
    let create_target = temp.path().join("create-state");
    let create_stale = temp.path().join(".create-state.tmp-42000-2");
    fs::write(&create_stale, b"older failed create").expect("seed create stale temp");
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let error = write_rooted_atomic_expected(
            &root_guard,
            &create_target,
            b"current create",
            governance_rooted_fs::ExpectedFile::Missing,
        )
        .expect_err("Missing writes must quarantine a stale create before mutation");
        assert!(
            error
                .to_string()
                .contains(GOVERNANCE_PUBLICATION_RECOVERY_QUARANTINE_DIR),
            "unexpected create quarantine error: {error}"
        );
        assert!(!create_target.exists());
        assert!(!create_stale.exists());
        assert_eq!(
            fs::read(recovery_quarantine_path(temp.path()).join("mutable-state-recovery-000000"))
                .expect("read isolated create temp"),
            b"older failed create"
        );
        clear_recovery_quarantine_offline(temp.path());
    }
    #[cfg(windows)]
    {
        write_rooted_atomic_expected(
            &root_guard,
            &create_target,
            b"current create",
            governance_rooted_fs::ExpectedFile::Missing,
        )
        .expect("Windows removes the exact opened stale temp before create");
        assert_eq!(
            fs::read(&create_target).expect("read created target"),
            b"current create"
        );
        assert!(!create_stale.exists());
    }
}
#[test]
fn filesystem_publisher_rejects_legacy_authority_temp_without_online_cleanup() {
    for legacy_name in [
        format!(".{GOVERNANCE_PUBLICATION_STATE_FILE}.tmp-42000-1"),
        format!(".{GOVERNANCE_PUBLICATION_STATE_FILE}.tmp-bad"),
        format!(".{GOVERNANCE_PUBLICATION_STATE_FILE}.retained-v1-bad"),
    ] {
        let temp = tempdir().expect("tempdir");
        let stale_temp = temp.path().join(&legacy_name);
        fs::write(&stale_temp, b"interrupted authoritative state")
            .expect("seed interrupted authority temp");
        let error = startup_error(temp.path(), "legacy authority temporary must fail closed");
        assert!(
            error
                .to_string()
                .contains("legacy governance publication authority"),
            "unexpected legacy authority error: {error}"
        );
        assert!(
            stale_temp.exists(),
            "startup must leave unsupported authority state for deliberate offline handling"
        );
        assert!(
            !temp
                .path()
                .join(GOVERNANCE_PUBLICATION_STORE_DIR_V1)
                .exists(),
            "legacy rejection must precede typed-store creation for `{legacy_name}`"
        );
    }
}
#[test]
fn filesystem_publisher_persists_explicit_empty_authority_and_marker() {
    let temp = tempdir().expect("tempdir");
    let publisher = try_publisher(temp.path()).expect("initialize publication authority");
    assert!(
        temp.path()
            .join(GOVERNANCE_PUBLICATION_STORE_DIR_V1)
            .is_dir(),
        "a pristine root must gain an explicit typed authority"
    );
    assert!(
        !temp.path().join(GOVERNANCE_PUBLICATION_STATE_FILE).exists(),
        "the retired flat-file authority must not be recreated"
    );
    assert_eq!(
        fs::read(temp.path().join(GOVERNANCE_PUBLICATION_INITIALIZED_FILE))
            .expect("read initialization marker"),
        GOVERNANCE_PUBLICATION_INITIALIZED_BODY
    );
    let state = read_publication_state_fixture(temp.path());
    assert_eq!(json_u64(&state, "generation"), Some(0));
    drop(publisher);
    let reader = GovernanceFilesystemRootGuard::capture_source(temp.path())
        .expect("retain read-only publication root");
    let snapshot = load_governance_publication_snapshot_v1(&reader)
        .expect("load typed publication reader snapshot")
        .expect("typed publication authority exists");
    assert_eq!(snapshot.store_identity().0, 1);
    assert_ne!(snapshot.store_identity().1, [0; 32]);
    let value: JsonValue = json::from_slice(snapshot.canonical_bytes())
        .expect("decode canonical publication reader bytes");
    assert_eq!(json_u64(&value, "generation"), Some(0));
}
#[test]
fn governance_publication_readers_reject_logical_store_generation_drift() {
    for substituted_generation in [7_u64, u64::MAX] {
        let temp = tempdir().expect("tempdir");
        let publisher = try_publisher(temp.path()).expect("initialize publication authority");
        let (mut state, snapshot) =
            read_governance_publication_state(&publisher.publication_state_store)
                .expect("load initial typed authority");
        state.insert("generation".into(), JsonValue::from(substituted_generation));
        let bytes = json::to_json_pretty(&JsonValue::Object(state))
            .expect("encode canonical substituted authority")
            .into_bytes();
        compare_and_swap_governance_two_slot_store_v1(
            &publisher.publication_state_store,
            &snapshot,
            &bytes,
            "substituted governance publication authority",
        )
        .expect("commit internally valid substituted authority");
        let error = read_governance_publication_state(&publisher.publication_state_store)
            .expect_err("publisher read boundary must reject generation drift");
        assert!(
            error
                .to_string()
                .contains("publication generation does not match its fixed-store generation"),
            "unexpected publisher drift error: {error}"
        );
        drop(publisher);
        let reader = GovernanceFilesystemRootGuard::capture_source(temp.path())
            .expect("retain read-only publication root");
        let error = load_governance_publication_snapshot_v1(&reader)
            .expect_err("public read boundary must reject generation drift");
        assert!(
            error
                .to_string()
                .contains("publication generation does not match its fixed-store generation"),
            "unexpected reader drift error: {error}"
        );
    }
}
#[test]
fn filesystem_publisher_restart_preserves_typed_authority_generation() {
    let temp = tempdir().expect("tempdir");
    let publisher = try_publisher(temp.path()).expect("initialize publication authority");
    let (settlement, encoded) = sample_settlement();
    publisher
        .publish_deal_settlement(&settlement, &encoded)
        .expect("advance typed publication authority");
    let before = read_publication_state_fixture(temp.path());
    assert_eq!(
        before.get("generation").and_then(JsonValue::as_u64),
        Some(1)
    );
    drop(publisher);
    drop(try_publisher(temp.path()).expect("restart accepts canonical typed authority"));
    assert_eq!(
        read_publication_state_fixture(temp.path()),
        before,
        "startup must not mutate the committed typed generation"
    );
    assert!(
        !temp.path().join(GOVERNANCE_PUBLICATION_STATE_FILE).exists(),
        "restart must not recreate the retired flat-file authority"
    );
}
#[test]
fn filesystem_publisher_rejects_missing_authority_without_deleting_history() {
    let temp = tempdir().expect("tempdir");
    let publisher = committed_settlement_publisher(temp.path());
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
    fs::remove_dir_all(temp.path().join(GOVERNANCE_PUBLICATION_STORE_DIR_V1))
        .expect("remove typed authority fixture");
    let error = startup_error(
        temp.path(),
        "missing initialized authority must fail closed",
    );
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert!(
        error
            .to_string()
            .contains("publication state is missing from an initialized root")
    );
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
    let publisher = committed_settlement_publisher(temp.path());
    let (encoded_path, _) = only_published_source_paths(temp.path(), "deal_settlement");
    drop(publisher);
    let substituted = b"substituted committed source";
    fs::write(&encoded_path, substituted).expect("substitute committed source");
    fs::write(
        digest_sidecar_path_for(&encoded_path),
        format!("{}\n", blake3::hash(substituted).to_hex()),
    )
    .expect("substitute matching unauthoritative sidecar");
    let error = startup_error(
        temp.path(),
        "authority-bound source corruption must fail startup",
    );
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
    let publisher = committed_settlement_publisher(temp.path());
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
    let error = startup_error(
        temp.path(),
        "authority-bound CAR corruption must fail startup",
    );
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
    let publisher = committed_settlement_publisher(temp.path());
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
    let error = startup_error(
        temp.path(),
        "startup must reject a missing committed artifact",
    );
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
        message.contains("regular file")
            || message.contains("reparse")
            || message.contains("symbolic link"),
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
    let metadata = json_object(&value, "metadata").expect("metadata");
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
    let publisher = try_publisher(temp.path()).expect("publisher");
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
    let metadata = json_object(&value, "metadata").expect("metadata");
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
        json_str(&value, "schema"),
        Some("sorafs.reputation_snapshot.metadata.v1")
    );
    assert!(
        value.get("signed_snapshot").is_none(),
        "the canonical payload belongs only in payload.to"
    );
    let metadata = json_object(&value, "metadata").expect("snapshot metadata");
    assert_eq!(
        metadata.get("encoded_len").and_then(JsonValue::as_u64),
        Some(GOVERNANCE_CAR_SOURCE_ENCODED_MAX_BYTES as u64)
    );
    assert!(
        !metadata.contains_key("encoded_base64"),
        "JSON metadata must not duplicate the canonical encoded payload"
    );
}
