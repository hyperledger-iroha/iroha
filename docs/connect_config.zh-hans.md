---
lang: zh-hans
direction: ltr
source: docs/connect_config.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8fa39bf0a08fb4c04d85c207bdffa287aabac979cc0496b54866b775154356e7
source_last_modified: "2026-01-05T18:22:23.392756+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## Torii 连接配置

Iroha Torii 公开可选的 WalletConnect 风格的 WebSocket 端点和最小的节点内中继
当 `connect` Cargo 功能启用时（默认）。运行时行为在配置中被控制：

- 设置 `connect.enabled=false` 以禁用所有连接路由 (`/v1/connect/*`)。
- 保留 `true`（默认）以启用 WS 会话端点和 `/v1/connect/status`。

环境覆盖（用户配置→实际配置）：

- `CONNECT_ENABLED`（布尔值；默认值：`true`）
- `CONNECT_WS_MAX_SESSIONS`（使用大小；默认值：`10000`）
- `CONNECT_WS_PER_IP_MAX_SESSIONS`（使用大小；默认值：`10`）
- `CONNECT_WS_RATE_PER_IP_PER_MIN`（u32；默认值：`120`）
- `CONNECT_FRAME_MAX_BYTES`（使用大小；默认值：`64000`）
- `CONNECT_SESSION_BUFFER_MAX_BYTES`（使用大小；默认值：`262144`）
- `CONNECT_PING_INTERVAL_MS`（持续时间；默认值：`30000`）
- `CONNECT_PING_MISS_TOLERANCE`（u32；默认值：`3`）
- `CONNECT_PING_MIN_INTERVAL_MS`（持续时间；默认值：`15000`）
- `CONNECT_DEDUPE_CAP`（使用大小；默认值：`8192`）
- `CONNECT_RELAY_ENABLED`（布尔值；默认值：`true`）
- `CONNECT_RELAY_STRATEGY`（字符串；默认值：`"broadcast"`）
- `CONNECT_P2P_TTL_HOPS`（u8；默认值：`0`）

注意事项：

- `CONNECT_SESSION_TTL_MS` 和 `CONNECT_DEDUPE_TTL_MS` 在用户配置中使用持续时间文字
  映射到实际的 `session_ttl` 和 `dedupe_ttl` 字段。
- `CONNECT_WS_PER_IP_MAX_SESSIONS=0` 禁用每 IP 会话上限。
- `CONNECT_WS_RATE_PER_IP_PER_MIN=0` 禁用每 IP 握手速率限制器。
- 心跳强制将配置的间隔限制为浏览器友好的最小值 (`ping_min_interval_ms`)；
  服务器在关闭 WebSocket 之前可以容忍 `ping_miss_tolerance` 次连续错过的 pong，并且
  增加 `connect.ping_miss_total` 指标。
- 在运行时禁用时 (`connect.enabled=false`)，Connect WS 和状态路由不会
  已注册；对 `/v1/connect/ws` 和 `/v1/connect/status` 的请求返回 404。
- 服务器需要客户端为 `/v1/connect/session` 提供 `sid`（base64url 或十六进制，32 字节）。
  它不再生成后备 `sid`。

另请参阅：`crates/iroha_config/src/parameters/{user,actual}.rs` 和默认值
`crates/iroha_config/src/parameters/defaults.rs`（模块 `connect`）。