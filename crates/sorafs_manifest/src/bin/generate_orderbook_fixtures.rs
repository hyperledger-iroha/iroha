//! Generates deterministic SoraFS orderbook and settlement fixtures.

use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use hex::encode;
use norito::{
    core::NoritoSerialize,
    json::{Map, Value, to_string_pretty},
};
use sorafs_manifest::{
    ByteRangeV1, ORDERBOOK_CANCEL_VERSION_V1, ORDERBOOK_ORDER_VERSION_V1,
    ORDERBOOK_TRADE_EVENT_VERSION_V1, OrderCancelReasonV1, OrderCancelV1, OrderRequestV1,
    OrderSideV1, OrderTierV1, OrderbookSignatureV1, SETTLEMENT_CHANNEL_VERSION_V1,
    SETTLEMENT_RECEIPT_VERSION_V1, SettlementChannelStatusV1, SettlementChannelV1,
    SettlementReceiptV1, SignatureAlgorithm, TradeEventV1, XorAmount,
};

fn main() -> Result<(), Box<dyn Error>> {
    let fixture_dir = PathBuf::from("fixtures/sorafs_manifest/orderbook");
    fs::create_dir_all(&fixture_dir)?;

    let order = OrderRequestV1 {
        version: ORDERBOOK_ORDER_VERSION_V1,
        order_id: id(0x71),
        side: OrderSideV1::Bid,
        tier: OrderTierV1::Hot,
        price_per_gib: XorAmount::from_micro(1_250_000),
        quantity_gib: 64,
        remaining_gib: 64,
        owner_account: b"buyer@sora".to_vec(),
        expiry_unix: 1_800_000_000,
        nonce: 7,
        maker_fee_bps: 10,
        taker_fee_bps: 15,
        signature: signature(),
    };
    order.validate()?;

    let cancel = OrderCancelV1 {
        version: ORDERBOOK_CANCEL_VERSION_V1,
        order_id: order.order_id,
        owner_account: order.owner_account.clone(),
        reason: OrderCancelReasonV1::OwnerRequested,
        nonce: 8,
        signature: signature(),
    };
    cancel.validate()?;

    let trade = TradeEventV1 {
        version: ORDERBOOK_TRADE_EVENT_VERSION_V1,
        trade_id: id(0x83),
        maker_order_id: order.order_id,
        taker_order_id: id(0x72),
        tier: order.tier,
        price_per_gib: order.price_per_gib,
        filled_gib: 16,
        maker_fee: XorAmount::from_micro(100),
        taker_fee: XorAmount::from_micro(150),
        timestamp_unix: 1_700_000_100,
    };
    trade.validate()?;

    let channel = SettlementChannelV1 {
        version: SETTLEMENT_CHANNEL_VERSION_V1,
        channel_id: id(0x82),
        trade_id: trade.trade_id,
        buyer_account: order.owner_account.clone(),
        provider_id: id(0x10),
        total_bytes: 1_048_576,
        remaining_bytes: 1_048_576,
        xor_locked: XorAmount::from_micro(1_000_000),
        status: SettlementChannelStatusV1::Open,
        opened_at_unix: 1_700_000_110,
        updated_at_unix: 1_700_000_110,
    };
    channel.validate()?;

    let receipt = SettlementReceiptV1 {
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
        xor_debited: XorAmount::from_micro(100),
        provider_credit: XorAmount::from_micro(90),
        fee_amount: XorAmount::from_micro(10),
        issued_at_unix: 1_700_000_120,
        settlement_signature: signature(),
    };
    receipt.validate()?;

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

    Ok(())
}

fn id(seed: u8) -> [u8; 32] {
    [seed; 32]
}

fn signature() -> OrderbookSignatureV1 {
    OrderbookSignatureV1 {
        algorithm: SignatureAlgorithm::Ed25519,
        public_key: vec![0xD7; 32],
        signature: vec![0x57; 64],
    }
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
        "price_per_gib_micro_xor".into(),
        Value::from(order.price_per_gib.as_micro().to_string()),
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
        "price_per_gib_micro_xor".into(),
        Value::from(trade.price_per_gib.as_micro().to_string()),
    );
    map.insert("filled_gib".into(), Value::from(trade.filled_gib));
    map.insert(
        "maker_fee_micro_xor".into(),
        Value::from(trade.maker_fee.as_micro().to_string()),
    );
    map.insert(
        "taker_fee_micro_xor".into(),
        Value::from(trade.taker_fee.as_micro().to_string()),
    );
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
        "xor_locked_micro_xor".into(),
        Value::from(channel.xor_locked.as_micro().to_string()),
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
        "xor_debited_micro_xor".into(),
        Value::from(receipt.xor_debited.as_micro().to_string()),
    );
    map.insert(
        "provider_credit_micro_xor".into(),
        Value::from(receipt.provider_credit.as_micro().to_string()),
    );
    map.insert(
        "fee_amount_micro_xor".into(),
        Value::from(receipt.fee_amount.as_micro().to_string()),
    );
    map.insert("issued_at_unix".into(), Value::from(receipt.issued_at_unix));
    map.insert(
        "settlement_signature".into(),
        signature_json(&receipt.settlement_signature),
    );
    Value::Object(map)
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
