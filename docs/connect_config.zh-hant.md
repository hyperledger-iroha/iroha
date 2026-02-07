---
lang: zh-hant
direction: ltr
source: docs/connect_config.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8fa39bf0a08fb4c04d85c207bdffa287aabac979cc0496b54866b775154356e7
source_last_modified: "2026-01-05T18:22:23.392756+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## Torii 連接配置

Iroha Torii 公開可選的 WalletConnect 風格的 WebSocket 端點和最小的節點內中繼
當 `connect` Cargo 功能啟用時（默認）。運行時行為在配置中被控制：

- 設置 `connect.enabled=false` 以禁用所有連接路由 (`/v1/connect/*`)。
- 保留 `true`（默認）以啟用 WS 會話端點和 `/v1/connect/status`。

環境覆蓋（用戶配置→實際配置）：

- `CONNECT_ENABLED`（布爾值；默認值：`true`）
- `CONNECT_WS_MAX_SESSIONS`（使用大小；默認值：`10000`）
- `CONNECT_WS_PER_IP_MAX_SESSIONS`（使用大小；默認值：`10`）
- `CONNECT_WS_RATE_PER_IP_PER_MIN`（u32；默認值：`120`）
- `CONNECT_FRAME_MAX_BYTES`（使用大小；默認值：`64000`）
- `CONNECT_SESSION_BUFFER_MAX_BYTES`（使用大小；默認值：`262144`）
- `CONNECT_PING_INTERVAL_MS`（持續時間；默認值：`30000`）
- `CONNECT_PING_MISS_TOLERANCE`（u32；默認值：`3`）
- `CONNECT_PING_MIN_INTERVAL_MS`（持續時間；默認值：`15000`）
- `CONNECT_DEDUPE_CAP`（使用大小；默認值：`8192`）
- `CONNECT_RELAY_ENABLED`（布爾值；默認值：`true`）
- `CONNECT_RELAY_STRATEGY`（字符串；默認值：`"broadcast"`）
- `CONNECT_P2P_TTL_HOPS`（u8；默認值：`0`）

注意事項：

- `CONNECT_SESSION_TTL_MS` 和 `CONNECT_DEDUPE_TTL_MS` 在用戶配置中使用持續時間文字
  映射到實際的 `session_ttl` 和 `dedupe_ttl` 字段。
- `CONNECT_WS_PER_IP_MAX_SESSIONS=0` 禁用每 IP 會話上限。
- `CONNECT_WS_RATE_PER_IP_PER_MIN=0` 禁用每 IP 握手速率限制器。
- 心跳強制將配置的間隔限制為瀏覽器友好的最小值 (`ping_min_interval_ms`)；
  服務器在關閉 WebSocket 之前可以容忍 `ping_miss_tolerance` 次連續錯過的 pong，並且
  增加 `connect.ping_miss_total` 指標。
- 在運行時禁用時 (`connect.enabled=false`)，Connect WS 和狀態路由不會
  已註冊；對 `/v1/connect/ws` 和 `/v1/connect/status` 的請求返回 404。
- 服務器需要客戶端為 `/v1/connect/session` 提供 `sid`（base64url 或十六進制，32 字節）。
  它不再生成後備 `sid`。

另請參閱：`crates/iroha_config/src/parameters/{user,actual}.rs` 和默認值
`crates/iroha_config/src/parameters/defaults.rs`（模塊 `connect`）。