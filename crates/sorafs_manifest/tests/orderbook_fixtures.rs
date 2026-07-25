#![allow(unexpected_cfgs)]

//! Round-trip and cross-SDK outcome coverage for committed SoraFS orderbook fixtures.

use std::{fs, path::Path};

use assert_cmd::cargo::cargo_bin_cmd;
use sorafs_manifest::{
    BYTES_PER_GIB, ORDERBOOK_CANCEL_VERSION_V1, ORDERBOOK_ORDER_VERSION_V1,
    ORDERBOOK_RUNTIME_SNAPSHOT_VERSION_V1, ORDERBOOK_TRADE_EVENT_VERSION_V1, OrderCancelV1,
    OrderRequestV1, OrderbookRuntimeSnapshotV1, OrderbookValidationPayloadKindV1,
    SETTLEMENT_CHANNEL_VERSION_V1, SETTLEMENT_RECEIPT_VERSION_V1, SettlementChannelV1,
    SettlementReceiptV1, TradeEventV1, derive_orderbook_order_id_v1, trade_escrow_requirement_v1,
    validate_orderbook_payload_bytes, verify_order_cancel_signature_v1,
    verify_order_request_signature_v1, verify_settlement_receipt_signature_v1,
};
use tempfile::tempdir;

const FIXTURES_ROOT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/sorafs_manifest/orderbook"
);

fn read_fixture_bytes(name: &str) -> Vec<u8> {
    let path = format!("{FIXTURES_ROOT}/{name}.to");
    fs::read(&path).unwrap_or_else(|err| panic!("failed to read {path}: {err}"))
}

fn regenerate_fixtures(root: &Path) {
    let output = cargo_bin_cmd!("generate_orderbook_fixtures")
        .current_dir(root)
        .output()
        .expect("run deterministic orderbook fixture generator");
    assert!(
        output.status.success(),
        "fixture generator failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

fn assert_outcome_fixture(name: &str, actual: &sorafs_manifest::ValidationOutcomeV1) {
    let path = format!("{FIXTURES_ROOT}/{name}");
    let expected =
        fs::read_to_string(&path).unwrap_or_else(|err| panic!("failed to read {path}: {err}"));
    let actual = format!(
        "{}\n",
        norito::json::to_string_pretty(actual).expect("serialize validation outcome")
    );
    assert_eq!(actual, expected, "validation outcome fixture drifted");
}

fn assert_json_hex_matches(name: &str, bytes: &[u8]) {
    let path = format!("{FIXTURES_ROOT}/{name}.json");
    let json_text =
        fs::read_to_string(&path).unwrap_or_else(|err| panic!("failed to read {path}: {err}"));
    let json_value =
        norito::json::parse_value(&json_text).expect("fixture commentary must be valid JSON");
    let norito_hex = json_value
        .get("norito_bytes_hex")
        .and_then(|value| value.as_str())
        .expect("fixture commentary must contain `norito_bytes_hex` string");
    let norito_bytes =
        hex::decode(norito_hex).expect("fixture commentary must contain valid hex payload");
    assert_eq!(norito_bytes, bytes, "`norito_bytes_hex` drifted");
}

#[test]
fn order_request_fixture_decodes_and_validates() {
    let bytes = read_fixture_bytes("order_request_v1");
    let order: OrderRequestV1 =
        norito::decode_from_bytes(&bytes).expect("order request fixture should decode");
    order
        .validate()
        .expect("order request fixture must validate");
    verify_order_request_signature_v1(&order).expect("order request signature must verify");
    assert_eq!(order.version, ORDERBOOK_ORDER_VERSION_V1);
    assert_eq!(
        order.order_id,
        derive_orderbook_order_id_v1(&order.owner_account, order.nonce)
    );
    assert_eq!(order.quantity_gib, 64);
    assert_eq!(order.remaining_gib, 64);
    assert_eq!(
        norito::to_bytes(&order).expect("fixture should re-encode"),
        bytes
    );
    assert_json_hex_matches("order_request_v1", &bytes);
}

#[test]
fn order_cancel_fixture_decodes_and_validates() {
    let bytes = read_fixture_bytes("order_cancel_v1");
    let cancel: OrderCancelV1 =
        norito::decode_from_bytes(&bytes).expect("order cancel fixture should decode");
    cancel
        .validate()
        .expect("order cancel fixture must validate");
    verify_order_cancel_signature_v1(&cancel).expect("order cancel signature must verify");
    assert_eq!(cancel.version, ORDERBOOK_CANCEL_VERSION_V1);
    assert_eq!(
        cancel.order_id,
        derive_orderbook_order_id_v1(&cancel.owner_account, 7)
    );
    assert_eq!(
        norito::to_bytes(&cancel).expect("fixture should re-encode"),
        bytes
    );
    assert_json_hex_matches("order_cancel_v1", &bytes);
}

#[test]
fn trade_event_fixture_decodes_and_validates() {
    let bytes = read_fixture_bytes("trade_event_v1");
    let trade: TradeEventV1 =
        norito::decode_from_bytes(&bytes).expect("trade event fixture should decode");
    trade.validate().expect("trade event fixture must validate");
    assert_eq!(trade.version, ORDERBOOK_TRADE_EVENT_VERSION_V1);
    assert_eq!(trade.trade_id, [0x83; 32]);
    assert_ne!(trade.maker_order_id, trade.taker_order_id);
    assert_eq!(
        norito::to_bytes(&trade).expect("fixture should re-encode"),
        bytes
    );
    assert_json_hex_matches("trade_event_v1", &bytes);
}

#[test]
fn settlement_channel_fixture_decodes_and_validates() {
    let bytes = read_fixture_bytes("settlement_channel_v1");
    let channel: SettlementChannelV1 =
        norito::decode_from_bytes(&bytes).expect("settlement channel fixture should decode");
    channel
        .validate()
        .expect("settlement channel fixture must validate");
    assert_eq!(channel.version, SETTLEMENT_CHANNEL_VERSION_V1);
    assert_eq!(channel.channel_id, [0x82; 32]);
    assert_eq!(channel.trade_id, [0x83; 32]);
    assert_eq!(
        norito::to_bytes(&channel).expect("fixture should re-encode"),
        bytes
    );
    assert_json_hex_matches("settlement_channel_v1", &bytes);
}

#[test]
fn settlement_receipt_fixture_decodes_and_validates() {
    let bytes = read_fixture_bytes("settlement_receipt_v1");
    let receipt: SettlementReceiptV1 =
        norito::decode_from_bytes(&bytes).expect("settlement receipt fixture should decode");
    receipt
        .validate()
        .expect("settlement receipt fixture must validate");
    verify_settlement_receipt_signature_v1(&receipt)
        .expect("settlement receipt signature must verify");
    assert_eq!(receipt.version, SETTLEMENT_RECEIPT_VERSION_V1);
    assert_eq!(receipt.receipt_id, [0x81; 32]);
    assert_eq!(receipt.channel_id, [0x82; 32]);
    assert_eq!(receipt.trade_id, [0x83; 32]);
    assert_eq!(receipt.bytes_delivered, 256);
    assert_eq!(
        norito::to_bytes(&receipt).expect("fixture should re-encode"),
        bytes
    );
    assert_json_hex_matches("settlement_receipt_v1", &bytes);
}

#[test]
fn runtime_snapshot_fixture_decodes_and_validates() {
    let bytes = read_fixture_bytes("runtime_snapshot_v1");
    let snapshot: OrderbookRuntimeSnapshotV1 =
        norito::decode_from_bytes(&bytes).expect("runtime snapshot fixture should decode");
    snapshot
        .validate()
        .expect("runtime snapshot fixture must validate");
    for entry in &snapshot.open_orders {
        verify_order_request_signature_v1(&entry.order)
            .expect("snapshot open-order signature must verify");
    }
    for receipt in &snapshot.settlement_receipts {
        verify_settlement_receipt_signature_v1(receipt)
            .expect("snapshot settlement-receipt signature must verify");
    }
    assert_eq!(snapshot.version, ORDERBOOK_RUNTIME_SNAPSHOT_VERSION_V1);
    assert_eq!(snapshot.next_sequence, 4);
    assert_eq!(snapshot.generated_at_unix, 1_700_000_130);
    assert_eq!(snapshot.owner_nonce_high_waters.len(), 1);
    assert_eq!(
        snapshot.owner_nonce_high_waters[0].owner_account,
        b"provider@sora"
    );
    assert_eq!(snapshot.owner_nonce_high_waters[0].highest_nonce, 9);
    assert_eq!(snapshot.open_orders.len(), 1);
    assert_eq!(
        snapshot.open_orders[0].order.order_id,
        derive_orderbook_order_id_v1(
            &snapshot.open_orders[0].order.owner_account,
            snapshot.open_orders[0].order.nonce,
        )
    );
    assert_eq!(snapshot.open_orders[0].sequence, 3);
    assert_eq!(snapshot.trades.len(), 1);
    assert_eq!(snapshot.trades[0].trade_id, [0x83; 32]);
    assert_eq!(snapshot.settlement_channels.len(), 1);
    assert_eq!(snapshot.settlement_channels[0].channel_id, [0x82; 32]);
    let expected_total_bytes = snapshot.trades[0].filled_gib * BYTES_PER_GIB;
    assert_eq!(
        snapshot.settlement_channels[0].total_bytes,
        expected_total_bytes
    );
    assert_eq!(
        snapshot.settlement_channels[0].remaining_bytes,
        expected_total_bytes - snapshot.settlement_receipts[0].bytes_delivered
    );
    let expected_escrow = trade_escrow_requirement_v1(&snapshot.trades[0])
        .expect("fixture trade escrow should compute")
        .checked_sub(&snapshot.settlement_receipts[0].xor_debited)
        .expect("fixture receipt debit should fit escrow");
    assert_eq!(snapshot.settlement_channels[0].xor_locked, expected_escrow);
    assert_eq!(
        snapshot.settlement_channels[0].updated_at_unix,
        1_700_000_120
    );
    assert_eq!(snapshot.settlement_receipts.len(), 1);
    assert_eq!(snapshot.settlement_receipts[0].receipt_id, [0x81; 32]);
    assert_eq!(snapshot.expired_order_ids, vec![[0x74; 32]]);
    assert_eq!(
        norito::to_bytes(&snapshot).expect("fixture should re-encode"),
        bytes
    );
    assert_json_hex_matches("runtime_snapshot_v1", &bytes);
}

#[test]
fn orderbook_reference_outcomes_match_cross_sdk_fixtures_exactly() {
    let order = read_fixture_bytes("order_request_v1");
    let order_outcome = validate_orderbook_payload_bytes(
        OrderbookValidationPayloadKindV1::OrderRequest,
        &order,
        "order_request_v1.to",
        123,
    );
    assert!(order_outcome.is_ok(), "{order_outcome:?}");
    assert_outcome_fixture("order_request_validation_outcome_v1.json", &order_outcome);

    for (payload_name, outcome_name, expected_code) in [
        (
            "negative/order_request_bad_signature_v1",
            "negative/order_request_bad_signature_validation_outcome_v1.json",
            "SFS-SIG-007",
        ),
        (
            "negative/order_request_trailing_bytes_v1",
            "negative/order_request_trailing_bytes_validation_outcome_v1.json",
            "SFS-NORITO-001",
        ),
    ] {
        let bytes = read_fixture_bytes(payload_name);
        let label = format!(
            "{}.to",
            payload_name
                .strip_prefix("negative/")
                .expect("negative fixture prefix")
        );
        let outcome = validate_orderbook_payload_bytes(
            OrderbookValidationPayloadKindV1::OrderRequest,
            &bytes,
            label,
            123,
        );
        assert!(!outcome.is_ok(), "{payload_name}: {outcome:?}");
        assert_eq!(outcome.code, expected_code, "{payload_name}: {outcome:?}");
        assert_outcome_fixture(outcome_name, &outcome);
    }
}

#[test]
fn orderbook_negative_vectors_preserve_signature_shape_and_break_canonical_encoding() {
    let canonical_bytes = read_fixture_bytes("order_request_v1");
    let forged_bytes = read_fixture_bytes("negative/order_request_bad_signature_v1");
    assert_eq!(
        canonical_bytes.len(),
        forged_bytes.len(),
        "signature forgery must preserve the canonical archive length"
    );
    let canonical: OrderRequestV1 =
        norito::decode_from_bytes(&canonical_bytes).expect("decode canonical order request");
    let forged: OrderRequestV1 =
        norito::decode_from_bytes(&forged_bytes).expect("decode forged order request");
    assert_eq!(canonical.signature.public_key, forged.signature.public_key);
    assert_eq!(
        canonical.signature.signature.len(),
        forged.signature.signature.len()
    );
    assert_eq!(
        canonical
            .signature
            .signature
            .iter()
            .zip(&forged.signature.signature)
            .filter(|(canonical, forged)| canonical != forged)
            .count(),
        1,
        "forged fixture must flip exactly one signature byte"
    );

    let trailing = read_fixture_bytes("negative/order_request_trailing_bytes_v1");
    assert_eq!(trailing.len(), canonical_bytes.len() + 1);
    assert_eq!(&trailing[..canonical_bytes.len()], canonical_bytes);
    assert_eq!(trailing.last(), Some(&0));
}

#[test]
fn orderbook_fixture_regeneration_is_byte_identical() {
    const FILES: [&str; 18] = [
        "order_request_v1.json",
        "order_request_v1.to",
        "order_cancel_v1.json",
        "order_cancel_v1.to",
        "trade_event_v1.json",
        "trade_event_v1.to",
        "settlement_channel_v1.json",
        "settlement_channel_v1.to",
        "settlement_receipt_v1.json",
        "settlement_receipt_v1.to",
        "runtime_snapshot_v1.json",
        "runtime_snapshot_v1.to",
        "order_request_validation_outcome_v1.json",
        "negative/order_request_bad_signature_v1.json",
        "negative/order_request_bad_signature_v1.to",
        "negative/order_request_bad_signature_validation_outcome_v1.json",
        "negative/order_request_trailing_bytes_v1.to",
        "negative/order_request_trailing_bytes_validation_outcome_v1.json",
    ];

    let first = tempdir().expect("create first fixture generation directory");
    let second = tempdir().expect("create second fixture generation directory");
    regenerate_fixtures(first.path());
    regenerate_fixtures(second.path());

    for name in FILES {
        let relative = Path::new("fixtures/sorafs_manifest/orderbook").join(name);
        let first_bytes = fs::read(first.path().join(&relative))
            .unwrap_or_else(|error| panic!("read first regenerated `{name}`: {error}"));
        let second_bytes = fs::read(second.path().join(&relative))
            .unwrap_or_else(|error| panic!("read second regenerated `{name}`: {error}"));
        let checked_in = fs::read(Path::new(FIXTURES_ROOT).join(name))
            .unwrap_or_else(|error| panic!("read checked-in `{name}`: {error}"));
        assert_eq!(
            first_bytes, second_bytes,
            "two regenerations diverged for `{name}`"
        );
        assert_eq!(
            first_bytes, checked_in,
            "regenerated bytes differ from checked-in `{name}`"
        );
    }
}
