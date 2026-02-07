---
lang: my
direction: ltr
source: docs/connect_config.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8fa39bf0a08fb4c04d85c207bdffa287aabac979cc0496b54866b775154356e7
source_last_modified: "2026-01-05T18:22:23.392756+00:00"
translation_last_reviewed: 2026-02-07
---

## Torii Connect Configuration

Iroha Torii exposes optional WalletConnect-style WebSocket endpoints and a minimal in-node relay
when the `connect` Cargo feature is enabled (default). The runtime behavior is gated at config:

- Set `connect.enabled=false` to disable all Connect routes (`/v1/connect/*`).
- Leave it `true` (default) to enable the WS session endpoints and `/v1/connect/status`.

Environment overrides (user config → actual config):

- `CONNECT_ENABLED` (bool; default: `true`)
- `CONNECT_WS_MAX_SESSIONS` (usize; default: `10000`)
- `CONNECT_WS_PER_IP_MAX_SESSIONS` (usize; default: `10`)
- `CONNECT_WS_RATE_PER_IP_PER_MIN` (u32; default: `120`)
- `CONNECT_FRAME_MAX_BYTES` (usize; default: `64000`)
- `CONNECT_SESSION_BUFFER_MAX_BYTES` (usize; default: `262144`)
- `CONNECT_PING_INTERVAL_MS` (duration; default: `30000`)
- `CONNECT_PING_MISS_TOLERANCE` (u32; default: `3`)
- `CONNECT_PING_MIN_INTERVAL_MS` (duration; default: `15000`)
- `CONNECT_DEDUPE_CAP` (usize; default: `8192`)
- `CONNECT_RELAY_ENABLED` (bool; default: `true`)
- `CONNECT_RELAY_STRATEGY` (string; default: `"broadcast"`)
- `CONNECT_P2P_TTL_HOPS` (u8; default: `0`)

Notes:

- `CONNECT_SESSION_TTL_MS` and `CONNECT_DEDUPE_TTL_MS` use duration literals in user config and
  map to actual `session_ttl` and `dedupe_ttl` fields.
- `CONNECT_WS_PER_IP_MAX_SESSIONS=0` disables the per-IP session cap.
- `CONNECT_WS_RATE_PER_IP_PER_MIN=0` disables the per-IP handshake rate limiter.
- Heartbeat enforcement clamps the configured interval to the browser-friendly minimum (`ping_min_interval_ms`);
  the server tolerates `ping_miss_tolerance` consecutive missed pongs before closing the WebSocket and
  increments the `connect.ping_miss_total` metric.
- When disabled at runtime (`connect.enabled=false`), Connect WS and status routes are not
  registered; requests to `/v1/connect/ws` and `/v1/connect/status` return 404.
- The server requires a client‑provided `sid` for `/v1/connect/session` (base64url or hex, 32 bytes).
  It no longer generates a fallback `sid`.

See also: `crates/iroha_config/src/parameters/{user,actual}.rs` and defaults in
`crates/iroha_config/src/parameters/defaults.rs` (module `connect`).
