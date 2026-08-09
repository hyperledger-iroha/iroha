// DA receipt-outcome and fixture-helper regressions.

#[test]
fn da_spool_rejection_response_allows_committed_receipt_outcomes() {
    for receipt_outcome in [
        ReceiptInsertOutcome::Stored {
            cursor_advanced: true,
        },
        ReceiptInsertOutcome::Duplicate {
            path: PathBuf::from("receipt.norito"),
        },
    ] {
        let mut batch = DaSpoolBatch::new();
        batch.push(DaSpoolAction::new("receipt_log", move || {
            Ok(DaSpoolActionOutput::ReceiptOutcome(receipt_outcome))
        }));
        let report = batch.execute_sync();

        assert!(
            da_spool_rejection_response(&report, ResponseFormat::Json).is_none(),
            "accepted receipt outcomes must not be converted into errors"
        );
    }
}

#[test]
fn da_spool_rejection_response_rejects_stale_receipt_outcome() {
    let mut batch = DaSpoolBatch::new();
    batch.push(DaSpoolAction::new("receipt_log", || {
        Ok(DaSpoolActionOutput::ReceiptOutcome(
            ReceiptInsertOutcome::StaleSequence { highest: 9 },
        ))
    }));
    let report = batch.execute_sync();
    let response = da_spool_rejection_response(&report, ResponseFormat::Json)
        .expect("stale receipt must produce a conflict response");

    assert_eq!(response.status(), StatusCode::CONFLICT);
}

#[test]
fn da_spool_rejection_response_rejects_sequence_gap_outcome() {
    let mut batch = DaSpoolBatch::new();
    batch.push(DaSpoolAction::new("receipt_log", || {
        Ok(DaSpoolActionOutput::ReceiptOutcome(
            ReceiptInsertOutcome::SequenceGap {
                expected_next: 10,
                observed: 12,
            },
        ))
    }));
    let report = batch.execute_sync();
    let response = da_spool_rejection_response(&report, ResponseFormat::Json)
        .expect("sequence gap receipt must produce a conflict response");

    assert_eq!(response.status(), StatusCode::CONFLICT);
}

#[test]
fn da_spool_rejection_response_rejects_receipt_conflict_outcome() {
    let mut batch = DaSpoolBatch::new();
    batch.push(DaSpoolAction::new("receipt_log", || {
        Ok(DaSpoolActionOutput::ReceiptOutcome(
            ReceiptInsertOutcome::ReceiptConflict {
                path: PathBuf::from("receipt.norito"),
            },
        ))
    }));
    let report = batch.execute_sync();
    let response = da_spool_rejection_response(&report, ResponseFormat::Json)
        .expect("receipt conflict must produce a conflict response");

    assert_eq!(response.status(), StatusCode::CONFLICT);
}

#[test]
fn da_spool_rejection_response_rejects_duplicate_fingerprint_conflict_outcome() {
    let mut batch = DaSpoolBatch::new();
    batch.push(DaSpoolAction::new("receipt_log", || {
        Ok(DaSpoolActionOutput::ReceiptOutcome(
            ReceiptInsertOutcome::DuplicateFingerprintConflict {
                path: PathBuf::from("receipt.norito"),
                expected: test_fingerprint(0xA1),
                observed: test_fingerprint(0xA2),
            },
        ))
    }));
    let report = batch.execute_sync();
    let response = da_spool_rejection_response(&report, ResponseFormat::Json)
        .expect("duplicate fingerprint conflict must produce a conflict response");

    assert_eq!(response.status(), StatusCode::CONFLICT);
}

#[test]
fn da_spool_rejection_response_rejects_manifest_conflict_outcome() {
    let mut batch = DaSpoolBatch::new();
    batch.push(DaSpoolAction::new("receipt_log", || {
        Ok(DaSpoolActionOutput::ReceiptOutcome(
            ReceiptInsertOutcome::ManifestConflict {
                expected: BlobDigest::new([1; 32]),
                observed: BlobDigest::new([2; 32]),
            },
        ))
    }));
    let report = batch.execute_sync();
    let response = da_spool_rejection_response(&report, ResponseFormat::Json)
        .expect("manifest conflict must produce a conflict response");

    assert_eq!(response.status(), StatusCode::CONFLICT);
}

#[test]
fn da_spool_rejection_response_rejects_missing_receipt_log_outcome() {
    let mut batch = DaSpoolBatch::new();
    batch.push(DaSpoolAction::new("manifest", || {
        Ok(DaSpoolActionOutput::None)
    }));
    let report = batch.execute_sync();
    let response = da_spool_rejection_response(&report, ResponseFormat::Json)
        .expect("missing receipt log outcome must fail closed");

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[test]
fn da_spool_rejection_response_rejects_spool_action_errors() {
    let mut batch = DaSpoolBatch::new();
    batch.push(DaSpoolAction::new("manifest", || {
        Err("disk full".to_owned())
    }));
    let report = batch.execute_sync();
    let response = da_spool_rejection_response(&report, ResponseFormat::Json)
        .expect("spool action errors must fail closed");

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

fn telemetry_handle_for_tests_with_profile(
    profile: TelemetryProfile,
) -> (Arc<Metrics>, MaybeTelemetry) {
    let metrics = test_metrics();
    let telemetry = Telemetry::new(metrics.clone(), true);
    let handle = MaybeTelemetry::from_profile(Some(telemetry), profile);
    (metrics, handle)
}

fn telemetry_handle_for_tests() -> (Arc<Metrics>, MaybeTelemetry) {
    telemetry_handle_for_tests_with_profile(TelemetryProfile::Operator)
}

fn test_metrics() -> Arc<Metrics> {
    enable_duplicate_metric_panic();
    Arc::new(Metrics::default())
}

fn enable_duplicate_metric_panic() {
    static INIT: LazyLock<()> = LazyLock::new(|| {
        #[allow(unsafe_code)]
        unsafe {
            std::env::set_var("IROHA_METRICS_PANIC_ON_DUPLICATE", "1");
        }
    });
    LazyLock::force(&INIT);
}

fn find_metric_line<'a>(dump: &'a str, prefix: &str) -> &'a str {
    dump.lines()
        .find(|line| line.starts_with(prefix))
        .unwrap_or_else(|| panic!("metric `{prefix}` not found\n{dump}"))
}

fn da_rent_metric_lines(dump: &str) -> Vec<String> {
    let mut lines: Vec<String> = dump
        .lines()
        .filter(|line| {
            line.starts_with("# HELP torii_da_")
                || line.starts_with("# TYPE torii_da_")
                || line.starts_with("torii_da_")
        })
        .filter(|line| {
            line.contains("_rent_")
                || line.contains("protocol_reserve_micro_total")
                || line.contains("provider_reward_micro_total")
                || line.contains("_pdp_bonus_micro_total")
                || line.contains("_potr_bonus_micro_total")
        })
        .map(str::to_owned)
        .collect();
    lines.sort();
    lines
}

fn parse_metric_value(line: &str) -> f64 {
    line.split_whitespace()
        .last()
        .unwrap_or_default()
        .parse::<f64>()
        .expect("metric value")
}

struct ChunkRecordFixture {
    file_name: String,
    offset: u64,
    length: u32,
    digest_hex: String,
}

fn load_chunk_record_fixture(name: &str) -> Vec<ChunkRecordFixture> {
    let path = fixtures_dir().join(name);
    let contents = fs::read_to_string(&path).unwrap_or_else(|err| {
        panic!("failed to read chunk fixture {}: {err}", path.display());
    });
    contents
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let mut parts = line.split_whitespace();
            let file_name = parts.next()?.to_string();
            let offset = parts
                .next()
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or_else(|| panic!("missing offset in fixture line `{line}`"));
            let length = parts
                .next()
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or_else(|| panic!("missing length in fixture line `{line}`"));
            let digest_hex = parts
                .next()
                .map(ToString::to_string)
                .unwrap_or_else(|| panic!("missing digest in fixture line `{line}`"));
            Some(ChunkRecordFixture {
                file_name,
                offset,
                length,
                digest_hex,
            })
        })
        .collect()
}

fn load_manifest_fixture(name: &str) -> Vec<u8> {
    let path = fixtures_dir().join(name);
    let contents = fs::read_to_string(&path).unwrap_or_else(|err| {
        panic!("failed to read manifest fixture {}: {err}", path.display());
    });
    hex::decode(contents.trim()).expect("fixture must be valid hex")
}

fn load_manifest_json_fixture(name: &str) -> Value {
    let path = fixtures_dir().join(name);
    let contents = fs::read_to_string(&path).unwrap_or_else(|err| {
        panic!(
            "failed to read manifest JSON fixture {}: {err}",
            path.display()
        );
    });
    json::from_str(&contents).expect("fixture must be valid Norito JSON")
}

fn write_manifest_fixture_bundle(
    case: &ManifestFixtureCase,
    context: &ManifestFixtureContext,
) -> std::io::Result<()> {
    let manifest_dir = fixtures_dir().join("manifests").join(case.slug);
    fs::create_dir_all(&manifest_dir)?;
    let hex_path = manifest_dir.join("manifest.norito.hex");
    let hex_text = format!("{}\n", hex::encode(&context.artifacts.encoded));
    fs::write(hex_path, hex_text)?;
    let manifest_value =
        json::to_value(&context.artifacts.manifest).expect("serialize manifest as JSON value");
    let json_text = json::to_string_pretty(&manifest_value).expect("render manifest JSON fixture");
    fs::write(manifest_dir.join("manifest.json"), format!("{json_text}\n"))?;
    Ok(())
}

fn write_chunk_record_fixture(
    path: &Path,
    records: &[PersistedChunkRecord],
    total_bytes: u64,
) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = fs::File::create(path)?;
    writeln!(file, "# file_name offset length digest_hex")?;
    for record in records {
        writeln!(
            file,
            "{} {} {} {}",
            record.file_name,
            record.offset,
            record.length,
            hex::encode(record.digest)
        )?;
    }
    writeln!(file, "# total_bytes {total_bytes}")?;
    Ok(())
}

fn format_base_id(
    lane_id: LaneId,
    epoch: u64,
    sequence: u64,
    ticket: &StorageTicketId,
    fingerprint: &ReplayFingerprint,
) -> String {
    let lane_hex = format!("{:08x}", lane_id.as_u32());
    let epoch_hex = format!("{:016x}", epoch);
    let sequence_hex = format!("{:016x}", sequence);
    let ticket_hex = hex::encode(ticket.as_ref());
    let fingerprint_hex = hex::encode(fingerprint.as_bytes());
    format!("{lane_hex}-{epoch_hex}-{sequence_hex}-{ticket_hex}-{fingerprint_hex}")
}

fn fixtures_dir() -> PathBuf {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/da/ingest");
    base.canonicalize()
        .expect("fixtures/da/ingest directory must exist")
}
