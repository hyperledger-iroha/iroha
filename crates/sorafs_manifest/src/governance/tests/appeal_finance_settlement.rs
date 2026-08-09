fn sample_appeal_finance_settlement_receipt() -> SoraFsAppealFinanceSettlementReceiptV1 {
    SoraFsAppealFinanceSettlementReceiptV1 {
        version: SORAFS_APPEAL_FINANCE_SETTLEMENT_RECEIPT_VERSION_V1,
        receipt_id: [0x52; 16],
        case_id: "case-42".to_string(),
        round_id: Some("round-1".to_string()),
        generated_at_unix_ms: 1_800_000_032_000,
        finalized_block_height: 42,
        finalized_block_hash: [0x43; 32],
        appeal_finance_config_version: "baseline-v1".to_string(),
        appeal_finance_policy_digest: [0x44; 32],
        outcome: SoraFsAppealFinanceOutcomeV1::Frivolous,
        escrow_id_hex: "11".repeat(32),
        payer_account: "payer-account".to_string(),
        destination_account: "escrow-account".to_string(),
        release_authority_account: Some("release-authority".to_string()),
        submitted_step: "drawdown_non_refund".to_string(),
        required_authority: "release-authority".to_string(),
        amount_xor: "420".parse().expect("canonical XOR quantity"),
        tx_hash_hex: "22".repeat(32),
        reconciliation_digest_hex: "33".repeat(32),
        reconciliation_status: "settled".to_string(),
        observed_lifecycle_status: "drawn_down".to_string(),
        observed_remaining_xor: "0".parse().expect("canonical XOR quantity"),
        deposit_xor: "420".parse().expect("canonical XOR quantity"),
        refund_xor: "0".parse().expect("canonical XOR quantity"),
        treasury_xor: "210".parse().expect("canonical XOR quantity"),
        held_xor: "210".parse().expect("canonical XOR quantity"),
        panel_size: 7,
        configured_signer_count: 1,
    }
}

#[test]
fn governance_payload_accepts_appeal_finance_settlement_receipt() {
    let receipt = sample_appeal_finance_settlement_receipt();
    let payload = GovernanceLogPayloadV1::AppealFinanceSettlementReceipt(receipt);

    payload
        .validate(1_800_000_032)
        .expect("settlement receipt payload validates");
}

#[test]
fn appeal_finance_settlement_receipt_accepts_canonical_bounds_and_applied_states() {
    let mut bounded = sample_appeal_finance_settlement_receipt();
    bounded.case_id = "c".repeat(APPEAL_FINANCE_SETTLEMENT_RECEIPT_IDENTIFIER_MAX_BYTES_V1);
    bounded.round_id = Some("r".repeat(APPEAL_FINANCE_SETTLEMENT_RECEIPT_IDENTIFIER_MAX_BYTES_V1));
    bounded.payer_account = "p".repeat(APPEAL_FINANCE_SETTLEMENT_RECEIPT_ACCOUNT_MAX_BYTES_V1);
    bounded.destination_account =
        "d".repeat(APPEAL_FINANCE_SETTLEMENT_RECEIPT_ACCOUNT_MAX_BYTES_V1);
    bounded.release_authority_account =
        Some("a".repeat(APPEAL_FINANCE_SETTLEMENT_RECEIPT_ACCOUNT_MAX_BYTES_V1));
    bounded.required_authority = "s".repeat(APPEAL_FINANCE_SETTLEMENT_RECEIPT_ACCOUNT_MAX_BYTES_V1);
    bounded.appeal_finance_config_version = format!(
        "{}-v1",
        "a".repeat(APPEAL_FINANCE_SETTLEMENT_RECEIPT_CONFIG_VERSION_MAX_BYTES_V1 - 3)
    );
    bounded.validate().expect("maximum canonical bounds");

    for (step, reconciliation, lifecycle) in [
        ("drawdown_non_refund", "awaiting_refund_cancel", "locked"),
        ("drawdown_non_refund", "settled", "drawn_down"),
        ("cancel_refund", "settled", "cancelled"),
    ] {
        let mut receipt = sample_appeal_finance_settlement_receipt();
        receipt.submitted_step = step.to_string();
        receipt.reconciliation_status = reconciliation.to_string();
        receipt.observed_lifecycle_status = lifecycle.to_string();
        receipt
            .validate()
            .unwrap_or_else(|error| panic!("{step}/{reconciliation}/{lifecycle}: {error}"));
    }
}

#[test]
fn appeal_finance_settlement_receipt_rejects_noncanonical_visible_identifiers() {
    fn assert_invalid_label(
        receipt: SoraFsAppealFinanceSettlementReceiptV1,
        field: &'static str,
        max_bytes: usize,
    ) {
        assert_eq!(
            receipt.validate(),
            Err(
                SoraFsAppealFinanceSettlementReceiptValidationError::InvalidLabel {
                    field,
                    max_bytes,
                }
            )
        );
    }

    let mut receipt = sample_appeal_finance_settlement_receipt();
    receipt.case_id = "case 42".to_string();
    assert_invalid_label(
        receipt,
        "case_id",
        APPEAL_FINANCE_SETTLEMENT_RECEIPT_IDENTIFIER_MAX_BYTES_V1,
    );

    let mut receipt = sample_appeal_finance_settlement_receipt();
    receipt.round_id = Some("round-é".to_string());
    assert_invalid_label(
        receipt,
        "round_id",
        APPEAL_FINANCE_SETTLEMENT_RECEIPT_IDENTIFIER_MAX_BYTES_V1,
    );

    let mut receipt = sample_appeal_finance_settlement_receipt();
    receipt.payer_account = "payer#account".to_string();
    assert_invalid_label(
        receipt,
        "payer_account",
        APPEAL_FINANCE_SETTLEMENT_RECEIPT_ACCOUNT_MAX_BYTES_V1,
    );

    let mut receipt = sample_appeal_finance_settlement_receipt();
    receipt.destination_account =
        "d".repeat(APPEAL_FINANCE_SETTLEMENT_RECEIPT_ACCOUNT_MAX_BYTES_V1 + 1);
    assert_invalid_label(
        receipt,
        "destination_account",
        APPEAL_FINANCE_SETTLEMENT_RECEIPT_ACCOUNT_MAX_BYTES_V1,
    );

    let mut receipt = sample_appeal_finance_settlement_receipt();
    receipt.release_authority_account = Some("release\nauthority".to_string());
    assert_invalid_label(
        receipt,
        "release_authority_account",
        APPEAL_FINANCE_SETTLEMENT_RECEIPT_ACCOUNT_MAX_BYTES_V1,
    );

    let mut receipt = sample_appeal_finance_settlement_receipt();
    receipt.required_authority = " authority".to_string();
    assert_invalid_label(
        receipt,
        "required_authority",
        APPEAL_FINANCE_SETTLEMENT_RECEIPT_ACCOUNT_MAX_BYTES_V1,
    );
}

#[test]
fn appeal_finance_settlement_receipt_requires_canonical_config_version() {
    for version in [
        "baseline-v1",
        "baseline-revision-v2",
        "baseline-rotated-v2",
        "appeal-finance-v10",
    ] {
        let mut receipt = sample_appeal_finance_settlement_receipt();
        receipt.appeal_finance_config_version = version.to_string();
        receipt
            .validate()
            .unwrap_or_else(|error| panic!("{version}: {error}"));
    }

    for version in [
        "baseline-v0",
        "baseline-v01",
        "Baseline-v1",
        "baseline_v1",
        "baseline-v1-revision-2",
        "baseline--revision-v2",
        "baseline-v2 ",
    ] {
        let mut receipt = sample_appeal_finance_settlement_receipt();
        receipt.appeal_finance_config_version = version.to_string();
        assert_eq!(
            receipt.validate(),
            Err(
                SoraFsAppealFinanceSettlementReceiptValidationError::InvalidFinanceConfigVersion {
                    max_bytes: APPEAL_FINANCE_SETTLEMENT_RECEIPT_CONFIG_VERSION_MAX_BYTES_V1,
                }
            ),
            "{version}"
        );
    }

    let mut oversized = sample_appeal_finance_settlement_receipt();
    oversized.appeal_finance_config_version = format!(
        "{}-v1",
        "a".repeat(APPEAL_FINANCE_SETTLEMENT_RECEIPT_CONFIG_VERSION_MAX_BYTES_V1 - 2)
    );
    assert_eq!(
        oversized.validate(),
        Err(
            SoraFsAppealFinanceSettlementReceiptValidationError::InvalidFinanceConfigVersion {
                max_bytes: APPEAL_FINANCE_SETTLEMENT_RECEIPT_CONFIG_VERSION_MAX_BYTES_V1,
            }
        )
    );
}

#[test]
fn appeal_finance_settlement_receipt_rejects_unfinalized_or_unknown_states() {
    let mut receipt = sample_appeal_finance_settlement_receipt();
    receipt.submitted_step = "treasury_release".to_string();
    assert_eq!(
        receipt.validate(),
        Err(SoraFsAppealFinanceSettlementReceiptValidationError::UnsupportedSubmittedStep)
    );

    for status in ["pending_forwarder_submission", "mismatch", "unknown"] {
        let mut receipt = sample_appeal_finance_settlement_receipt();
        receipt.reconciliation_status = status.to_string();
        assert_eq!(
                receipt.validate(),
                Err(
                    SoraFsAppealFinanceSettlementReceiptValidationError::UnsupportedReconciliationStatus
                ),
                "{status}"
            );
    }

    for status in ["funded", "expired", "unknown"] {
        let mut receipt = sample_appeal_finance_settlement_receipt();
        receipt.observed_lifecycle_status = status.to_string();
        assert_eq!(
            receipt.validate(),
            Err(SoraFsAppealFinanceSettlementReceiptValidationError::UnsupportedLifecycleStatus),
            "{status}"
        );
    }

    for (step, reconciliation, lifecycle) in [
        ("drawdown_non_refund", "settled", "locked"),
        (
            "drawdown_non_refund",
            "awaiting_refund_cancel",
            "drawn_down",
        ),
        ("cancel_refund", "awaiting_refund_cancel", "locked"),
        ("cancel_refund", "settled", "drawn_down"),
    ] {
        let mut receipt = sample_appeal_finance_settlement_receipt();
        receipt.submitted_step = step.to_string();
        receipt.reconciliation_status = reconciliation.to_string();
        receipt.observed_lifecycle_status = lifecycle.to_string();
        assert_eq!(
            receipt.validate(),
            Err(SoraFsAppealFinanceSettlementReceiptValidationError::InconsistentFinalizedState),
            "{step}/{reconciliation}/{lifecycle}"
        );
    }
}

#[test]
fn appeal_finance_settlement_receipt_rejects_zero_timestamp_and_panel() {
    let mut receipt = sample_appeal_finance_settlement_receipt();
    receipt.generated_at_unix_ms = 0;
    assert_eq!(
        receipt.validate(),
        Err(SoraFsAppealFinanceSettlementReceiptValidationError::MissingGeneratedAt)
    );

    let mut receipt = sample_appeal_finance_settlement_receipt();
    receipt.panel_size = 0;
    assert_eq!(
        receipt.validate(),
        Err(SoraFsAppealFinanceSettlementReceiptValidationError::InvalidPanelSize)
    );
}

#[test]
fn appeal_finance_settlement_receipt_requires_finalized_block_cursor() {
    let mut receipt = sample_appeal_finance_settlement_receipt();
    receipt.finalized_block_height = 0;
    assert_eq!(
        receipt.validate(),
        Err(SoraFsAppealFinanceSettlementReceiptValidationError::InvalidFinalizedBlockHeight)
    );

    let mut receipt = sample_appeal_finance_settlement_receipt();
    receipt.finalized_block_hash = [0; 32];
    assert_eq!(
        receipt.validate(),
        Err(SoraFsAppealFinanceSettlementReceiptValidationError::InvalidFinalizedBlockHash)
    );
}

#[test]
fn appeal_finance_settlement_receipt_canonical_digest_binds_finalized_cursor() {
    let receipt = sample_appeal_finance_settlement_receipt();
    let encoded = norito::to_bytes(&receipt).expect("encode receipt");
    let digest = blake3::hash(&encoded);

    let mut changed_height = receipt.clone();
    changed_height.finalized_block_height += 1;
    let changed_height_encoded =
        norito::to_bytes(&changed_height).expect("encode changed-height receipt");
    assert_ne!(changed_height_encoded, encoded);
    assert_ne!(blake3::hash(&changed_height_encoded), digest);

    let mut changed_hash = receipt;
    changed_hash.finalized_block_hash[0] ^= 0x01;
    let changed_hash_encoded =
        norito::to_bytes(&changed_hash).expect("encode changed-hash receipt");
    assert_ne!(changed_hash_encoded, encoded);
    assert_ne!(blake3::hash(&changed_hash_encoded), digest);
}

fn sample_orderbook_settlement_receipt() -> SettlementReceiptV1 {
    SettlementReceiptV1 {
        version: crate::SETTLEMENT_RECEIPT_VERSION_V1,
        receipt_id: [0x72; 32],
        channel_id: [0x73; 32],
        trade_id: [0x74; 32],
        range: crate::ByteRangeV1 {
            start: 0,
            end: crate::BYTES_PER_GIB,
        },
        chunk_hash: [0x75; 32],
        bytes_delivered: crate::BYTES_PER_GIB,
        xor_debited: crate::XorQuantity::try_from_micro(500)
            .expect("legacy micro-XOR value is representable"),
        provider_credit: crate::XorQuantity::try_from_micro(450)
            .expect("legacy micro-XOR value is representable"),
        fee_amount: crate::XorQuantity::try_from_micro(50)
            .expect("legacy micro-XOR value is representable"),
        issued_at_unix: 1_800_000_033,
        settlement_signature: crate::OrderbookSignatureV1 {
            algorithm: crate::provider_advert::SignatureAlgorithm::Ed25519,
            public_key: vec![0x76; 32],
            signature: vec![0x77; 64],
        },
    }
}

#[test]
fn governance_payload_accepts_orderbook_settlement_receipt() {
    let receipt = sample_orderbook_settlement_receipt();
    let payload = GovernanceLogPayloadV1::OrderbookSettlementReceipt(receipt);

    payload
        .validate(1_800_000_033)
        .expect("orderbook settlement receipt payload validates");
}

#[test]
fn appeal_finance_settlement_receipt_rejects_invalid_reconciliation_digest() {
    let mut receipt = sample_appeal_finance_settlement_receipt();
    receipt.reconciliation_digest_hex = "AA".repeat(32);

    let err = receipt
        .validate()
        .expect_err("uppercase digest rejected as non-canonical");
    assert_eq!(
        err,
        SoraFsAppealFinanceSettlementReceiptValidationError::InvalidHex {
            field: "reconciliation_digest_hex",
            expected_bytes: 32,
        }
    );
}

#[test]
fn appeal_finance_settlement_receipt_requires_exact_lowercase_hex_fields() {
    fn assert_invalid_hex(receipt: SoraFsAppealFinanceSettlementReceiptV1, field: &'static str) {
        assert_eq!(
            receipt.validate(),
            Err(
                SoraFsAppealFinanceSettlementReceiptValidationError::InvalidHex {
                    field,
                    expected_bytes: 32,
                }
            )
        );
    }

    let mut receipt = sample_appeal_finance_settlement_receipt();
    receipt.escrow_id_hex = "AA".repeat(32);
    assert_invalid_hex(receipt, "escrow_id_hex");

    let mut receipt = sample_appeal_finance_settlement_receipt();
    receipt.tx_hash_hex = "2".repeat(63);
    assert_invalid_hex(receipt, "tx_hash_hex");

    let mut receipt = sample_appeal_finance_settlement_receipt();
    receipt.reconciliation_digest_hex = format!("{}g", "3".repeat(63));
    assert_invalid_hex(receipt, "reconciliation_digest_hex");
}

#[test]
fn appeal_finance_settlement_receipt_rejects_zero_policy_digest() {
    let mut receipt = sample_appeal_finance_settlement_receipt();
    receipt.appeal_finance_policy_digest = [0; 32];

    let err = receipt
        .validate()
        .expect_err("zero governed policy digest rejected");
    assert_eq!(
        err,
        SoraFsAppealFinanceSettlementReceiptValidationError::InvalidFinancePolicyDigest
    );
}
