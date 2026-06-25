# SoraFS Orderbook Fixtures

This directory contains deterministic orderbook and streaming-settlement
fixtures for the SFM-2/SF-11 reference validators.

- `order_request_v1.to` / `order_request_v1.json` encode `OrderRequestV1`.
- `order_cancel_v1.to` / `order_cancel_v1.json` encode `OrderCancelV1`.
- `trade_event_v1.to` / `trade_event_v1.json` encode `TradeEventV1`.
- `settlement_channel_v1.to` / `settlement_channel_v1.json` encode
  `SettlementChannelV1`.
- `settlement_receipt_v1.to` / `settlement_receipt_v1.json` encode
  `SettlementReceiptV1`.
- `runtime_snapshot_v1.to` / `runtime_snapshot_v1.json` encode
  `OrderbookRuntimeSnapshotV1`.

Regenerate the fixtures with:

```sh
cargo run -p sorafs_manifest --bin generate_orderbook_fixtures
```

The fixture-directory bundle validator discovers all of these `.to` files when
run against `fixtures/sorafs_manifest`. The runtime snapshot can also be
validated directly with
`sorafs-validate orderbook --snapshot runtime_snapshot_v1.to`.
