<!-- Japanese translation of docs/connect_config.md -->

---
lang: ja
direction: ltr
source: docs/connect_config.md
status: complete
translator: manual
---

## Torii Connect 設定

Iroha Torii は、`connect` Cargo フィーチャ（デフォルトで有効）が有効化された場合に、WalletConnect 風の WebSocket エンドポイントと最小限のノード内リレーを公開します。ランタイムでの挙動は設定により制御されます。

- `connect.enabled=false` に設定すると Connect のすべてのルート（`/v1/connect/*`）が無効化されます。
- デフォルト値の `true` のままにすると WS セッションエンドポイントと `/v1/connect/status` が有効になります。

環境変数による上書き（ユーザー設定 → 実際の設定）:

- `CONNECT_ENABLED`（bool、既定値: `true`）
- `CONNECT_WS_MAX_SESSIONS`（usize、既定値: `10000`）
- `CONNECT_WS_PER_IP_MAX_SESSIONS`（usize、既定値: `10`）
- `CONNECT_WS_RATE_PER_IP_PER_MIN`（u32、既定値: `120`）
- `CONNECT_FRAME_MAX_BYTES`（usize、既定値: `64000`）
- `CONNECT_SESSION_BUFFER_MAX_BYTES`（usize、既定値: `262144`）
- `CONNECT_PING_INTERVAL_MS`（duration、既定値: `30000`）
- `CONNECT_PING_MISS_TOLERANCE`（u32、既定値: `3`）
- `CONNECT_PING_MIN_INTERVAL_MS`（duration、既定値: `15000`）
- `CONNECT_DEDUPE_CAP`（usize、既定値: `8192`）
- `CONNECT_RELAY_ENABLED`（bool、既定値: `true`）
- `CONNECT_RELAY_STRATEGY`（文字列、既定値: `"broadcast"`）
- `CONNECT_P2P_TTL_HOPS`（u8、既定値: `0`）

補足:

- `CONNECT_SESSION_TTL_MS` と `CONNECT_DEDUPE_TTL_MS` はユーザー設定で期間リテラルを使用し、実際の設定フィールド `session_ttl` と `dedupe_ttl` にマッピングされます。
- ハートビートの強制は設定されたインターバルをブラウザ向けの下限値（`ping_min_interval_ms`）でクランプし、`ping_miss_tolerance` 回連続で応答がない場合に WebSocket をクローズしてメトリクス `connect.ping_miss_total` を加算します。
- ランタイムで無効化されている場合（`connect.enabled=false`）、Connect の WS ルートとステータスルートは登録されません。`/v1/connect/ws` と `/v1/connect/status` へのリクエストは 404 を返します。
- サーバーは `/v1/connect/session` でクライアント提供の `sid` を要求します（base64url または 16 進数で 32 バイト）。フォールバック `sid` の生成は行われません。

参考: `crates/iroha_config/src/parameters/{user,actual}.rs` および `crates/iroha_config/src/parameters/defaults.rs`（`connect` モジュール）を参照してください。
