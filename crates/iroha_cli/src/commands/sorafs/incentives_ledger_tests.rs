// Ledger export, reconciliation assertions and their shared CLI fixtures.
fn sample_transfer_record(kind: TransferKind, amount: u32) -> LedgerTransferRecord {
    LedgerTransferRecord {
        relay_id: [0xAA; 32],
        epoch: 3,
        kind,
        dispute_id: None,
        amount: Quantity::from(amount),
        source_asset: AssetId::new(xor_asset_id(), sample_account_id("treasury")),
        destination: sample_account_id("relay"),
    }
}
test_items! {
fn incentive_quantity_parser_rejects_negative_amounts() {
    let error = parse_quantity_str("-1", "--requested-amount")
        .expect_err("negative reward quantities must be rejected");
    assert!(error.to_string().contains("non-negative quantity"));
}
fn ledger_export_schema_mismatch_reports_expected_and_actual() {
    const SCHEMA_OFFSET: usize = 4 + 1 + 1;
    let export = LedgerExportFile {
        version: LedgerExportFile::VERSION,
        transfers: vec![sample_transfer_record(TransferKind::Payout, 5)],
    };
    let mut bytes = to_bytes(&export).expect("encode ledger export");
    bytes[SCHEMA_OFFSET] ^= 0xFF;
    let expected = hex::encode(norito::schema::identity::frame_hash::<LedgerExportFile>());
    let actual = hex::encode(&bytes[SCHEMA_OFFSET..SCHEMA_OFFSET + 16]);
    let file = NamedTempFile::new().expect("temp file");
    fs::write(file.path(), bytes).expect("write ledger export");
    let err = read_ledger_export(file.path()).expect_err("schema mismatch should fail");
    let messages = err.chain().map(ToString::to_string).collect::<Vec<_>>();
    let combined = messages.join("\n");
    assert_compact! { combined.contains("schema mismatch"); "expected schema mismatch in error chain: {combined}" };
    assert_compact! { combined.contains("expected"); "expected schema hash detail in error chain: {combined}" };
    assert_compact! { combined.contains("got"); "expected actual schema hash detail in error chain: {combined}" };
    assert!(combined.contains(&format!("expected {expected}")));
    assert!(combined.contains(&format!("got {actual}")));
}
fn reconciliation_summary_builds_expected_counts() {
    let missing_record = sample_transfer_record(TransferKind::Payout, 100);
    let unexpected_record = sample_transfer_record(TransferKind::Credit, 25);
    let mismatch_expected = sample_transfer_record(TransferKind::Debit, 40);
    let mut mismatch_actual = sample_transfer_record(TransferKind::Debit, 35);
    mismatch_actual.destination = sample_account_id("alt-treasury");
    let mut invalid_amount_record = sample_transfer_record(TransferKind::Payout, 5);
    invalid_amount_record.amount = "340282366920938463463374607431768211456"
        .parse::<Quantity>()
        .expect("2^128 quantity");
    let report = LedgerReconciliationReport {
        total_expected_transfers: 3,
        matched_transfers: 1,
        expected_amount: 150_u64.into(),
        exported_amount: 70_u64.into(),
        missing_transfers: vec![ExpectedLedgerTransfer {
            record: missing_record.clone(),
        }],
        unexpected_transfers: vec![unexpected_record.clone()],
        mismatched_transfers: vec![LedgerTransferMismatch {
            expected: mismatch_expected.clone(),
            actual: mismatch_actual.clone(),
            reasons: vec![MismatchReason::Amount, MismatchReason::Destination],
        }],
        amount_arithmetic_errors: vec![LedgerAmountArithmeticError {
            source: LedgerAmountSource::Exported,
            record: invalid_amount_record.clone(),
        }],
    };
    let summary = ReconciliationReportSummary::from_report(&report);
    assert!(!summary.clean);
    assert_eq!(summary.matched_transfers, 1);
    assert_eq!(summary.total_expected_transfers, 3);
    assert_eq!(summary.missing_transfers.len(), 1);
    assert_eq!(summary.unexpected_transfers.len(), 1);
    assert_eq!(summary.mismatched_transfers.len(), 1);
    assert_eq_compact! { summary.missing_transfers[0].relay_id => relay_id_to_hex(missing_record.relay_id) };
    assert_eq_compact! { summary.unexpected_transfers[0].kind => transfer_kind_label(unexpected_record.kind) };
    assert_compact! { summary.mismatched_transfers[0].reasons.iter().any(|reason| reason == "amount") };
    assert_eq!(summary.amount_arithmetic_errors.len(), 1);
    assert_eq!(summary.amount_arithmetic_errors[0].source, "exported");
    assert_eq_compact! { summary.amount_arithmetic_errors[0].record.amount => invalid_amount_record.amount.to_string() };
    assert_eq_compact! { summary.amount_arithmetic_errors[0].record.amount_nanos => None };
    assert_compact! { summary.amount_arithmetic_errors[0].record.amount_conversion_error.is_some() };
}
}
fn sample_account_id(name: &str) -> AccountId {
    let mut hasher = Blake3Hasher::new();
    hasher.update(b"sorafs-sample-account");
    hasher.update(name.as_bytes());
    let digest = hasher.finalize();
    let mut seed = [0u8; 32];
    seed.copy_from_slice(digest.as_bytes());
    let signing = SigningKey::from_bytes(&seed);
    let verifying = signing.verifying_key();
    let public_key =
        PublicKey::from_bytes(Algorithm::Ed25519, verifying.as_bytes()).expect("public key");
    AccountId::new(public_key)
}
fn sample_account_literal(name: &str) -> String {
    let account = sample_account_id(name);
    account.to_string()
}
fn xor_asset_id() -> AssetDefinitionId {
    AssetDefinitionId::derive_from_components(
        iroha_data_model::domain::DomainId::try_new("sora", "universal").unwrap(),
        "xor".parse().unwrap(),
    )
}
fn sample_budget_id_hex() -> String {
    hex::encode(sample_budget_id())
}
fn sample_budget_id() -> [u8; 32] {
    [0x11_u8; 32]
}
fn write_reward_config_with_budget(budget_hex: Option<&str>) -> NamedTempFile {
    let mut config = sample_reward_config_json();
    let budget_value = budget_hex.map_or(Value::Null, |hex| Value::String(hex.to_string()));
    config
        .as_object_mut()
        .expect("sample reward config should be an object")
        .insert("budget_approval_id".to_string(), budget_value);
    let mut file = NamedTempFile::new().expect("config file");
    let bytes = norito::json::to_vec(&config).expect("encode config");
    file.write_all(&bytes).expect("write config");
    file
}
fn write_sample_reward_config_file() -> NamedTempFile {
    let mut file = NamedTempFile::new().expect("config file");
    let bytes = norito::json::to_vec(&sample_reward_config_json()).expect("encode config");
    file.write_all(&bytes).expect("write config");
    file
}
fn write_metrics_file(metrics: &RelayEpochMetricsV1) -> NamedTempFile {
    let mut file = NamedTempFile::new().expect("metrics file");
    let bytes = to_bytes(metrics).expect("encode metrics");
    file.write_all(&bytes).expect("write metrics");
    file
}
fn write_bond_file(bond: &RelayBondLedgerEntryV1) -> NamedTempFile {
    let mut file = NamedTempFile::new().expect("bond file");
    let bytes = to_bytes(bond).expect("encode bond");
    file.write_all(&bytes).expect("write bond");
    file
}
fn write_gc_manifest(
    root: &Path,
    manifest_id: &str,
    retention_epoch: u64,
    storage_class: ManifestStorageClass,
    payload_bytes: u64,
    car_bytes: u64,
) {
    let manifest = ManifestBuilder::new()
        .root_cid(vec![0x01, 0x02, 0x03])
        .dag_codec(DagCodecId(0x71))
        .chunking_profile(ChunkingProfileV1 {
            profile_id: ProfileId(7),
            namespace: "sorafs".into(),
            name: "sf1".into(),
            semver: "1.0.0".into(),
            min_size: 4096,
            target_size: 262_144,
            max_size: 524_288,
            break_mask: 0,
            multihash_code: BLAKE3_256_MULTIHASH_CODE,
            aliases: vec!["sf1".into()],
        })
        .chunk_digest_sha3_256([0xCD; 32])
        .por_root(if payload_bytes == 0 {
            sorafs_manifest::EMPTY_POR_ROOT_V1
        } else {
            [0xCE; 32]
        })
        .content_length(payload_bytes)
        .car_digest([0xAB; 32])
        .car_size(car_bytes)
        .pin_policy(PinPolicy {
            min_replicas: 1,
            storage_class,
            retention_epoch,
        })
        .build()
        .expect("build manifest");
    let bytes = to_bytes(&manifest).expect("encode manifest");
    let manifest_dir = root.join(SORAFS_MANIFEST_DIR).join(manifest_id);
    fs::create_dir_all(&manifest_dir).expect("create manifest dir");
    fs::write(manifest_dir.join(SORAFS_MANIFEST_FILE), bytes).expect("write manifest file");
}
fn read_state(path: &Path) -> IncentivesState {
    load_incentives_state(path).expect("decode incentives state")
}
fn initialize_incentives_state(config: &Path, state: &Path) -> TestContext {
    let args = IncentivesServiceInitArgs {
        state: state.to_path_buf(),
        config: config.to_path_buf(),
        treasury_account: sample_account_literal("treasury"),
        force: false,
    };
    let mut context = TestContext::new();
    args.run(&mut context).expect("init command runs");
    context
}
fn write_state_without_budget(path: &Path) {
    let config_file = write_reward_config_with_budget(None);
    let reward_config = read_reward_config(config_file.path()).expect("reward config");
    let state = IncentivesState::new(&reward_config, sample_account_id("treasury"));
    save_incentives_state(path, &state).expect("write incentives state");
}

#[test]
fn ledger_export_frame_round_trip_and_version_rejection_use_the_file_reader() {
    use norito::NoritoSchema as _;
    let export = LedgerExportFile {
        version: LedgerExportFile::VERSION,
        transfers: vec![sample_transfer_record(TransferKind::Payout, 5)],
    };
    assert_eq!(
        LedgerExportFile::nominal_name(),
        "iroha::commands::sorafs::LedgerExportFile"
    );
    assert_eq!(
        LedgerExportFile::frame_name(),
        LedgerExportFile::nominal_name()
    );
    let file = NamedTempFile::new().expect("ledger export file");
    let bytes = to_bytes(&export).expect("encode current ledger export frame");
    fs::write(file.path(), &bytes).expect("write current export");
    let decoded = read_ledger_export(file.path()).expect("read current ledger export");
    assert_eq!(decoded.version, export.version);
    assert_eq!(decoded.transfers, export.transfers);
    assert_eq!(to_bytes(&decoded).expect("re-encode ledger export"), bytes);
    for version in [0, LedgerExportFile::VERSION + 1] {
        let unsupported = LedgerExportFile {
            version,
            transfers: export.transfers.clone(),
        };
        fs::write(
            file.path(),
            to_bytes(&unsupported).expect("encode unsupported export"),
        )
        .expect("write unsupported export");
        let error = read_ledger_export(file.path()).expect_err("unsupported version must fail");
        assert!(
            error
                .to_string()
                .contains(&format!("unsupported ledger export version {version}"))
        );
    }
}
