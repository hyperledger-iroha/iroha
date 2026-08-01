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
- `order_request_validation_outcome_v1.json` is the complete canonical
  `ValidationOutcomeV1` for the signed request at `generated_at=123`.
- `negative/order_request_bad_signature_v1.*` carries a same-length Ed25519
  signature forgery and its exact `SFS-SIG-007` outcome.
- `negative/order_request_trailing_bytes_v1.to` is a noncanonical archive and
  is paired with its exact `SFS-NORITO-001` outcome.

All user-signed positive fixtures use deterministic, cryptographically valid
Ed25519 signatures.

The Rust, JavaScript/TypeScript, Python, Swift, Kotlin/JVM, Java Android, and
C# suites compare each complete positive and negative `ValidationOutcomeV1`
against the canonical JSON (byte-for-byte for string-returning APIs and after
canonical pretty serialization for object-returning APIs).

Regenerate the fixtures with:

```sh
cargo run --locked -p sorafs_manifest --features dev-tools --bin generate_orderbook_fixtures
```

The fixture-directory bundle validator discovers all of these `.to` files when
run against `fixtures/sorafs_manifest`.
