#![allow(unexpected_cfgs)]

//! Round-trip coverage for committed SoraFS orderbook fixtures.

use std::fs;

use sorafs_manifest::{
    ORDERBOOK_CANCEL_VERSION_V1, ORDERBOOK_ORDER_VERSION_V1, ORDERBOOK_RUNTIME_SNAPSHOT_VERSION_V1,
    ORDERBOOK_TRADE_EVENT_VERSION_V1, OrderCancelV1, OrderRequestV1, OrderbookRuntimeSnapshotV1,
    SETTLEMENT_CHANNEL_VERSION_V1, SETTLEMENT_RECEIPT_VERSION_V1, SettlementChannelV1,
    SettlementReceiptV1, TradeEventV1,
};

const FIXTURES_ROOT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/sorafs_manifest/orderbook"
);

fn read_fixture_bytes(name: &str) -> Vec<u8> {
    let path = format!("{FIXTURES_ROOT}/{name}.to");
    fs::read(&path).unwrap_or_else(|err| panic!("failed to read {path}: {err}"))
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
    assert_eq!(order.version, ORDERBOOK_ORDER_VERSION_V1);
    assert_eq!(order.order_id, [0x71; 32]);
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
    assert_eq!(cancel.version, ORDERBOOK_CANCEL_VERSION_V1);
    assert_eq!(cancel.order_id, [0x71; 32]);
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
    assert_eq!(snapshot.version, ORDERBOOK_RUNTIME_SNAPSHOT_VERSION_V1);
    assert_eq!(snapshot.next_sequence, 4);
    assert_eq!(snapshot.generated_at_unix, 1_700_000_130);
    assert_eq!(snapshot.open_orders.len(), 1);
    assert_eq!(snapshot.open_orders[0].order.order_id, [0x73; 32]);
    assert_eq!(snapshot.open_orders[0].sequence, 3);
    assert_eq!(snapshot.trades.len(), 1);
    assert_eq!(snapshot.trades[0].trade_id, [0x83; 32]);
    assert_eq!(snapshot.settlement_channels.len(), 1);
    assert_eq!(snapshot.settlement_channels[0].channel_id, [0x82; 32]);
    assert_eq!(snapshot.settlement_channels[0].remaining_bytes, 1_048_320);
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
