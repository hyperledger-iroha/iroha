//! Generates deterministic SoraFS orderbook and settlement fixtures.

use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use ed25519_dalek::SigningKey;
use hex::encode;
use norito::{
    core::NoritoSerialize,
    json::{Map, Value, to_string_pretty},
};
use sorafs_manifest::{
    ByteRangeV1, ORDERBOOK_CANCEL_VERSION_V1, ORDERBOOK_ORDER_VERSION_V1,
    ORDERBOOK_RUNTIME_SNAPSHOT_VERSION_V1, ORDERBOOK_TRADE_EVENT_VERSION_V1, OrderBookEntryV1,
    OrderCancelReasonV1, OrderCancelV1, OrderRequestV1, OrderSideV1, OrderTierV1,
    OrderbookOwnerNonceHighWaterV1, OrderbookRuntimeSnapshotV1, OrderbookSignatureV1,
    OrderbookValidationPayloadKindV1, SETTLEMENT_RECEIPT_VERSION_V1, SettlementChannelStatusV1,
    SettlementChannelV1, SettlementReceiptV1, SignatureAlgorithm, TradeEventV1,
    apply_settlement_receipt_v1, derive_orderbook_order_id_v1,
    open_settlement_channel_for_trade_v1, sign_order_cancel_ed25519_v1,
    sign_order_request_ed25519_v1, sign_settlement_receipt_ed25519_v1,
    validate_orderbook_payload_bytes, verify_order_cancel_signature_v1,
    verify_order_request_signature_v1, verify_settlement_receipt_signature_v1,
};

const VALIDATION_GENERATED_AT: u64 = 123;

fn main() -> Result<(), Box<dyn Error>> {
    let fixture_dir = PathBuf::from("fixtures/sorafs_manifest/orderbook");
    let negative_dir = fixture_dir.join("negative");
    fs::create_dir_all(&fixture_dir)?;
    fs::create_dir_all(&negative_dir)?;
    let signing_key = SigningKey::from_bytes(&[0xB7; 32]);

    let order_owner = b"buyer@sora".to_vec();
    let order_nonce = 7;
    let order = sign_order_request_ed25519_v1(
        OrderRequestV1 {
            version: ORDERBOOK_ORDER_VERSION_V1,
            order_id: derive_orderbook_order_id_v1(&order_owner, order_nonce),
            side: OrderSideV1::Bid,
            tier: OrderTierV1::Hot,
            price_per_gib: "1.25".parse().expect("canonical XOR quantity"),
            quantity_gib: 64,
            remaining_gib: 64,
            owner_account: order_owner,
            expiry_unix: 1_800_000_000,
            nonce: order_nonce,
            maker_fee_bps: 10,
            taker_fee_bps: 15,
            signature: empty_signature(&signing_key),
        },
        &signing_key,
    )?;
    verify_order_request_signature_v1(&order)?;

    let cancel = sign_order_cancel_ed25519_v1(
        OrderCancelV1 {
            version: ORDERBOOK_CANCEL_VERSION_V1,
            order_id: order.order_id,
            owner_account: order.owner_account.clone(),
            reason: OrderCancelReasonV1::OwnerRequested,
            nonce: 8,
            signature: empty_signature(&signing_key),
        },
        &signing_key,
    )?;
    verify_order_cancel_signature_v1(&cancel)?;

    let trade = TradeEventV1 {
        version: ORDERBOOK_TRADE_EVENT_VERSION_V1,
        trade_id: id(0x83),
        maker_order_id: order.order_id,
        taker_order_id: id(0x72),
        tier: order.tier,
        price_per_gib: order.price_per_gib.clone(),
        filled_gib: 16,
        maker_fee: "0.0001".parse().expect("canonical XOR quantity"),
        taker_fee: "0.00015".parse().expect("canonical XOR quantity"),
        timestamp_unix: 1_700_000_100,
    };
    trade.validate()?;

    let channel = open_settlement_channel_for_trade_v1(
        &trade,
        id(0x82),
        order.owner_account.clone(),
        id(0x10),
        1_700_000_110,
    )?;

    let receipt = sign_settlement_receipt_ed25519_v1(
        SettlementReceiptV1 {
            version: SETTLEMENT_RECEIPT_VERSION_V1,
            receipt_id: id(0x81),
            channel_id: channel.channel_id,
            trade_id: trade.trade_id,
            range: ByteRangeV1 {
                start: 128,
                end: 384,
            },
            chunk_hash: id(0x84),
            bytes_delivered: 256,
            xor_debited: "0.0001".parse().expect("canonical XOR quantity"),
            provider_credit: "0.00009".parse().expect("canonical XOR quantity"),
            fee_amount: "0.00001".parse().expect("canonical XOR quantity"),
            issued_at_unix: 1_700_000_120,
            settlement_signature: empty_signature(&signing_key),
        },
        &signing_key,
    )?;
    verify_settlement_receipt_signature_v1(&receipt)?;

    let snapshot_owner = b"provider@sora".to_vec();
    let snapshot_nonce = 9;
    let snapshot_open_order = sign_order_request_ed25519_v1(
        OrderRequestV1 {
            version: ORDERBOOK_ORDER_VERSION_V1,
            order_id: derive_orderbook_order_id_v1(&snapshot_owner, snapshot_nonce),
            side: OrderSideV1::Ask,
            tier: OrderTierV1::Hot,
            price_per_gib: "1.3".parse().expect("canonical XOR quantity"),
            quantity_gib: 32,
            remaining_gib: 24,
            owner_account: snapshot_owner,
            expiry_unix: 1_800_000_100,
            nonce: snapshot_nonce,
            maker_fee_bps: 10,
            taker_fee_bps: 15,
            signature: empty_signature(&signing_key),
        },
        &signing_key,
    )?;
    verify_order_request_signature_v1(&snapshot_open_order)?;

    let snapshot_channel = apply_settlement_receipt_v1(&channel, &receipt)?;
    let runtime_snapshot = OrderbookRuntimeSnapshotV1 {
        version: ORDERBOOK_RUNTIME_SNAPSHOT_VERSION_V1,
        next_sequence: 4,
        generated_at_unix: 1_700_000_130,
        owner_nonce_high_waters: vec![OrderbookOwnerNonceHighWaterV1 {
            owner_account: snapshot_open_order.owner_account.clone(),
            highest_nonce: snapshot_open_order.nonce,
        }],
        open_orders: vec![OrderBookEntryV1 {
            order: snapshot_open_order,
            sequence: 3,
        }],
        trades: vec![trade.clone()],
        settlement_channels: vec![snapshot_channel],
        settlement_receipts: vec![receipt.clone()],
        expired_order_ids: vec![id(0x74)],
    };
    runtime_snapshot.validate()?;

    write_norito_pair(
        &fixture_dir.join("order_request_v1"),
        &order,
        order_json(&order),
    )?;
    write_norito_pair(
        &fixture_dir.join("order_cancel_v1"),
        &cancel,
        cancel_json(&cancel),
    )?;
    write_norito_pair(
        &fixture_dir.join("trade_event_v1"),
        &trade,
        trade_json(&trade),
    )?;
    write_norito_pair(
        &fixture_dir.join("settlement_channel_v1"),
        &channel,
        channel_json(&channel),
    )?;
    write_norito_pair(
        &fixture_dir.join("settlement_receipt_v1"),
        &receipt,
        receipt_json(&receipt),
    )?;
    write_norito_pair(
        &fixture_dir.join("runtime_snapshot_v1"),
        &runtime_snapshot,
        runtime_snapshot_json(&runtime_snapshot),
    )?;

    let order_bytes = norito::to_bytes(&order)?;
    let order_outcome = validate_orderbook_payload_bytes(
        OrderbookValidationPayloadKindV1::OrderRequest,
        &order_bytes,
        "order_request_v1.to",
        VALIDATION_GENERATED_AT,
    );
    write_expected_outcome(
        &fixture_dir.join("order_request_validation_outcome_v1.json"),
        &order_outcome,
        true,
        "SFS-OK-000",
    )?;

    let mut forged_order = order.clone();
    *forged_order
        .signature
        .signature
        .first_mut()
        .ok_or("orderbook fixture signature must not be empty")? ^= 1;
    write_norito_pair(
        &negative_dir.join("order_request_bad_signature_v1"),
        &forged_order,
        order_json(&forged_order),
    )?;
    let forged_order_bytes = norito::to_bytes(&forged_order)?;
    let forged_outcome = validate_orderbook_payload_bytes(
        OrderbookValidationPayloadKindV1::OrderRequest,
        &forged_order_bytes,
        "order_request_bad_signature_v1.to",
        VALIDATION_GENERATED_AT,
    );
    write_expected_outcome(
        &negative_dir.join("order_request_bad_signature_validation_outcome_v1.json"),
        &forged_outcome,
        false,
        "SFS-SIG-007",
    )?;

    let mut trailing_order_bytes = order_bytes;
    trailing_order_bytes.push(0);
    fs::write(
        negative_dir.join("order_request_trailing_bytes_v1.to"),
        &trailing_order_bytes,
    )?;
    let trailing_outcome = validate_orderbook_payload_bytes(
        OrderbookValidationPayloadKindV1::OrderRequest,
        &trailing_order_bytes,
        "order_request_trailing_bytes_v1.to",
        VALIDATION_GENERATED_AT,
    );
    write_expected_outcome(
        &negative_dir.join("order_request_trailing_bytes_validation_outcome_v1.json"),
        &trailing_outcome,
        false,
        "SFS-NORITO-001",
    )?;

    Ok(())
}

fn id(seed: u8) -> [u8; 32] {
    [seed; 32]
}

fn empty_signature(signing_key: &SigningKey) -> OrderbookSignatureV1 {
    OrderbookSignatureV1 {
        algorithm: SignatureAlgorithm::Ed25519,
        public_key: signing_key.verifying_key().to_bytes().to_vec(),
        signature: Vec::new(),
    }
}

fn write_expected_outcome(
    path: &Path,
    outcome: &sorafs_manifest::ValidationOutcomeV1,
    expected_ok: bool,
    expected_code: &str,
) -> Result<(), Box<dyn Error>> {
    if outcome.is_ok() != expected_ok || outcome.code != expected_code {
        return Err(format!(
            "generated orderbook outcome returned status_ok={} code={}, expected status_ok={expected_ok} code={expected_code}",
            outcome.is_ok(),
            outcome.code,
        )
        .into());
    }
    fs::write(path, format!("{}\n", to_string_pretty(outcome)?))?;
    Ok(())
}

fn write_norito_pair<T>(
    base_path: &Path,
    value: &T,
    mut json_value: Value,
) -> Result<(), Box<dyn Error>>
where
    T: NoritoSerialize,
{
    let bytes = norito::to_bytes(value)?;
    fs::write(base_path.with_extension("to"), &bytes)?;
    if let Value::Object(map) = &mut json_value {
        map.insert("norito_bytes_hex".into(), Value::from(encode(&bytes)));
    }
    let json = to_string_pretty(&json_value)?;
    fs::write(base_path.with_extension("json"), json)?;
    Ok(())
}

fn order_json(order: &OrderRequestV1) -> Value {
    let mut map = Map::new();
    map.insert("version".into(), Value::from(order.version));
    map.insert("order_id_hex".into(), Value::from(encode(order.order_id)));
    map.insert("side".into(), Value::from(order_side(order.side)));
    map.insert("tier".into(), Value::from(order_tier(order.tier)));
    map.insert(
        "price_per_gib".into(),
        Value::from(order.price_per_gib.to_string()),
    );
    map.insert("quantity_gib".into(), Value::from(order.quantity_gib));
    map.insert("remaining_gib".into(), Value::from(order.remaining_gib));
    map.insert(
        "owner_account".into(),
        Value::from(String::from_utf8_lossy(&order.owner_account).to_string()),
    );
    map.insert("expiry_unix".into(), Value::from(order.expiry_unix));
    map.insert("nonce".into(), Value::from(order.nonce));
    map.insert("maker_fee_bps".into(), Value::from(order.maker_fee_bps));
    map.insert("taker_fee_bps".into(), Value::from(order.taker_fee_bps));
    map.insert("signature".into(), signature_json(&order.signature));
    Value::Object(map)
}

fn cancel_json(cancel: &OrderCancelV1) -> Value {
    let mut map = Map::new();
    map.insert("version".into(), Value::from(cancel.version));
    map.insert("order_id_hex".into(), Value::from(encode(cancel.order_id)));
    map.insert(
        "owner_account".into(),
        Value::from(String::from_utf8_lossy(&cancel.owner_account).to_string()),
    );
    map.insert("reason".into(), Value::from(cancel_reason(cancel.reason)));
    map.insert("nonce".into(), Value::from(cancel.nonce));
    map.insert("signature".into(), signature_json(&cancel.signature));
    Value::Object(map)
}

fn trade_json(trade: &TradeEventV1) -> Value {
    let mut map = Map::new();
    map.insert("version".into(), Value::from(trade.version));
    map.insert("trade_id_hex".into(), Value::from(encode(trade.trade_id)));
    map.insert(
        "maker_order_id_hex".into(),
        Value::from(encode(trade.maker_order_id)),
    );
    map.insert(
        "taker_order_id_hex".into(),
        Value::from(encode(trade.taker_order_id)),
    );
    map.insert("tier".into(), Value::from(order_tier(trade.tier)));
    map.insert(
        "price_per_gib".into(),
        Value::from(trade.price_per_gib.to_string()),
    );
    map.insert("filled_gib".into(), Value::from(trade.filled_gib));
    map.insert("maker_fee".into(), Value::from(trade.maker_fee.to_string()));
    map.insert("taker_fee".into(), Value::from(trade.taker_fee.to_string()));
    map.insert("timestamp_unix".into(), Value::from(trade.timestamp_unix));
    Value::Object(map)
}

fn channel_json(channel: &SettlementChannelV1) -> Value {
    let mut map = Map::new();
    map.insert("version".into(), Value::from(channel.version));
    map.insert(
        "channel_id_hex".into(),
        Value::from(encode(channel.channel_id)),
    );
    map.insert("trade_id_hex".into(), Value::from(encode(channel.trade_id)));
    map.insert(
        "buyer_account".into(),
        Value::from(String::from_utf8_lossy(&channel.buyer_account).to_string()),
    );
    map.insert(
        "provider_id_hex".into(),
        Value::from(encode(channel.provider_id)),
    );
    map.insert("total_bytes".into(), Value::from(channel.total_bytes));
    map.insert(
        "remaining_bytes".into(),
        Value::from(channel.remaining_bytes),
    );
    map.insert(
        "xor_locked".into(),
        Value::from(channel.xor_locked.to_string()),
    );
    map.insert("status".into(), Value::from(channel_status(channel.status)));
    map.insert("opened_at_unix".into(), Value::from(channel.opened_at_unix));
    map.insert(
        "updated_at_unix".into(),
        Value::from(channel.updated_at_unix),
    );
    Value::Object(map)
}

fn receipt_json(receipt: &SettlementReceiptV1) -> Value {
    let mut map = Map::new();
    map.insert("version".into(), Value::from(receipt.version));
    map.insert(
        "receipt_id_hex".into(),
        Value::from(encode(receipt.receipt_id)),
    );
    map.insert(
        "channel_id_hex".into(),
        Value::from(encode(receipt.channel_id)),
    );
    map.insert("trade_id_hex".into(), Value::from(encode(receipt.trade_id)));
    map.insert("range_start".into(), Value::from(receipt.range.start));
    map.insert("range_end".into(), Value::from(receipt.range.end));
    map.insert(
        "chunk_hash_hex".into(),
        Value::from(encode(receipt.chunk_hash)),
    );
    map.insert(
        "bytes_delivered".into(),
        Value::from(receipt.bytes_delivered),
    );
    map.insert(
        "xor_debited".into(),
        Value::from(receipt.xor_debited.to_string()),
    );
    map.insert(
        "provider_credit".into(),
        Value::from(receipt.provider_credit.to_string()),
    );
    map.insert(
        "fee_amount".into(),
        Value::from(receipt.fee_amount.to_string()),
    );
    map.insert("issued_at_unix".into(), Value::from(receipt.issued_at_unix));
    map.insert(
        "settlement_signature".into(),
        signature_json(&receipt.settlement_signature),
    );
    Value::Object(map)
}

fn runtime_snapshot_json(snapshot: &OrderbookRuntimeSnapshotV1) -> Value {
    let mut map = Map::new();
    map.insert("version".into(), Value::from(snapshot.version));
    map.insert("next_sequence".into(), Value::from(snapshot.next_sequence));
    map.insert(
        "generated_at_unix".into(),
        Value::from(snapshot.generated_at_unix),
    );
    map.insert(
        "owner_nonce_high_water_count".into(),
        Value::from(snapshot.owner_nonce_high_waters.len() as u64),
    );
    map.insert(
        "owner_nonce_high_waters".into(),
        Value::Array(
            snapshot
                .owner_nonce_high_waters
                .iter()
                .map(owner_nonce_high_water_json)
                .collect(),
        ),
    );
    map.insert(
        "open_order_count".into(),
        Value::from(snapshot.open_orders.len() as u64),
    );
    map.insert(
        "trade_count".into(),
        Value::from(snapshot.trades.len() as u64),
    );
    map.insert(
        "settlement_channel_count".into(),
        Value::from(snapshot.settlement_channels.len() as u64),
    );
    map.insert(
        "settlement_receipt_count".into(),
        Value::from(snapshot.settlement_receipts.len() as u64),
    );
    map.insert(
        "expired_order_count".into(),
        Value::from(snapshot.expired_order_ids.len() as u64),
    );
    map.insert(
        "open_orders".into(),
        Value::Array(
            snapshot
                .open_orders
                .iter()
                .map(open_order_entry_json)
                .collect(),
        ),
    );
    map.insert(
        "trade_ids_hex".into(),
        bytes32_array_json(snapshot.trades.iter().map(|trade| trade.trade_id)),
    );
    map.insert(
        "settlement_channel_ids_hex".into(),
        bytes32_array_json(
            snapshot
                .settlement_channels
                .iter()
                .map(|channel| channel.channel_id),
        ),
    );
    map.insert(
        "settlement_receipt_ids_hex".into(),
        bytes32_array_json(
            snapshot
                .settlement_receipts
                .iter()
                .map(|receipt| receipt.receipt_id),
        ),
    );
    map.insert(
        "expired_order_ids_hex".into(),
        bytes32_array_json(snapshot.expired_order_ids.iter().copied()),
    );
    Value::Object(map)
}

fn owner_nonce_high_water_json(entry: &OrderbookOwnerNonceHighWaterV1) -> Value {
    let mut map = Map::new();
    map.insert(
        "owner_account".into(),
        Value::from(String::from_utf8_lossy(&entry.owner_account).to_string()),
    );
    map.insert("highest_nonce".into(), Value::from(entry.highest_nonce));
    Value::Object(map)
}

fn open_order_entry_json(entry: &OrderBookEntryV1) -> Value {
    let mut map = Map::new();
    map.insert("sequence".into(), Value::from(entry.sequence));
    map.insert(
        "order_id_hex".into(),
        Value::from(encode(entry.order.order_id)),
    );
    map.insert("side".into(), Value::from(order_side(entry.order.side)));
    map.insert("tier".into(), Value::from(order_tier(entry.order.tier)));
    map.insert(
        "price_per_gib".into(),
        Value::from(entry.order.price_per_gib.to_string()),
    );
    map.insert("quantity_gib".into(), Value::from(entry.order.quantity_gib));
    map.insert(
        "remaining_gib".into(),
        Value::from(entry.order.remaining_gib),
    );
    map.insert(
        "owner_account".into(),
        Value::from(String::from_utf8_lossy(&entry.order.owner_account).to_string()),
    );
    map.insert("expiry_unix".into(), Value::from(entry.order.expiry_unix));
    Value::Object(map)
}

fn bytes32_array_json(items: impl IntoIterator<Item = [u8; 32]>) -> Value {
    Value::Array(
        items
            .into_iter()
            .map(|item| Value::from(encode(item)))
            .collect(),
    )
}

fn signature_json(signature: &OrderbookSignatureV1) -> Value {
    let mut map = Map::new();
    map.insert("algorithm".into(), Value::from("ed25519"));
    map.insert(
        "public_key_hex".into(),
        Value::from(encode(&signature.public_key)),
    );
    map.insert(
        "signature_hex".into(),
        Value::from(encode(&signature.signature)),
    );
    Value::Object(map)
}

fn order_side(side: OrderSideV1) -> &'static str {
    match side {
        OrderSideV1::Bid => "bid",
        OrderSideV1::Ask => "ask",
    }
}

fn order_tier(tier: OrderTierV1) -> &'static str {
    match tier {
        OrderTierV1::Hot => "hot",
        OrderTierV1::Warm => "warm",
        OrderTierV1::Archive => "archive",
    }
}

fn cancel_reason(reason: OrderCancelReasonV1) -> &'static str {
    match reason {
        OrderCancelReasonV1::OwnerRequested => "owner_requested",
        OrderCancelReasonV1::Expired => "expired",
        OrderCancelReasonV1::Governance => "governance",
        OrderCancelReasonV1::Replaced => "replaced",
    }
}

fn channel_status(status: SettlementChannelStatusV1) -> &'static str {
    match status {
        SettlementChannelStatusV1::Open => "open",
        SettlementChannelStatusV1::Closing => "closing",
        SettlementChannelStatusV1::Closed => "closed",
        SettlementChannelStatusV1::Breached => "breached",
        SettlementChannelStatusV1::Refunded => "refunded",
    }
}
