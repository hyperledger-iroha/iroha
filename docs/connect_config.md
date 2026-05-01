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
- `CONNECT_RELAY_STRATEGY` (string; default: `"broadcast"`; allowed: `"broadcast"`, `"local_only"`; compatibility aliases: `"local-only"`, `"local"`)
- `CONNECT_P2P_TTL_HOPS` (u8; default: `0`; `0` disables cross-node rebroadcast)

Notes:

- `CONNECT_SESSION_TTL_MS` and `CONNECT_DEDUPE_TTL_MS` use duration literals in user config and
  map to actual `session_ttl` and `dedupe_ttl` fields.
- `CONNECT_WS_PER_IP_MAX_SESSIONS=0` disables the per-IP session cap.
- `CONNECT_WS_RATE_PER_IP_PER_MIN=0` disables the per-IP handshake rate limiter.
- `CONNECT_RELAY_STRATEGY="broadcast"` relays Connect frames through the Iroha node-to-node P2P network;
  `"local_only"` keeps relay traffic on the local node only. Compatibility aliases `"local-only"` and
  `"local"` normalize to `"local_only"`. Unknown strategy values are forced to `"local_only"` to avoid
  unintended relay behavior. Torii does not use centralized relay servers. P2P relay frames are
  authenticated with the session `token_relay`; frames with invalid MACs are dropped before dedupe or
  sequence state is updated.
- When broadcast relay is active and a session is created, Torii gossips a P2P
  session claim containing token authentication hashes and the relay MAC key to
  authenticated peers. This lets an app and wallet attach through different
  Torii nodes for the same session without putting raw app, wallet, or
  management tokens on the P2P wire.
- WebSocket ingress validates role→direction mapping (`app` must send `AppToWallet`, `wallet` must
  send `WalletToApp`). Mismatches terminate the session with `connect_role_direction_mismatch` and
  increment `connect.role_direction_mismatch_total`.
- Frames are deduplicated by `(sid, dir, seq)` before sequence checks. Duplicates increment
  `connect.dedupe_drops_total` and are dropped without advancing sequence state.
- Per direction, sequence numbers are strict and contiguous starting at `1`; non-contiguous frames
  terminate the session with `connect_sequence_violation` and increment
  `connect.sequence_violation_closes_total`.
- Public `/v1/connect/status` is redacted and does not expose per-IP session details. Add
  `?sid=<sid>` plus `Authorization: Bearer <token_management>` for token-gated per-session status.
- `/v1/connect/status` reports top-level P2P counters including `p2p_rebroadcasts_total`,
  `p2p_rebroadcast_skipped_total`, `p2p_auth_failures_total`,
  `p2p_ttl_drops_total`, `p2p_unknown_session_drops_total`,
  `p2p_session_claims_in_total`, `p2p_session_claims_installed_total`,
  `p2p_session_claim_conflicts_total`, `p2p_role_consumed_total`, and
  `p2p_session_terminated_total`.
- `/v1/connect/status.policy` includes both configured and effective relay mode:
  `relay_strategy` (normalized config), `relay_effective_strategy` (runtime behavior), and
  `relay_p2p_attached` (whether Torii currently has a P2P relay handle). This allows operators to
  confirm that cross-node forwarding is happening over decentralized node-to-node transport.
- Heartbeat enforcement clamps the configured interval to the browser-friendly minimum (`ping_min_interval_ms`);
  the server tolerates `ping_miss_tolerance` consecutive missed pongs before closing the WebSocket and
  increments the `connect.ping_miss_total` metric.
- When disabled at runtime (`connect.enabled=false`), Connect WS and status routes are not
  registered; requests to `/v1/connect/ws` and `/v1/connect/status` return 404.
- The server requires a client‑provided `sid` for `/v1/connect/session` (base64url or hex, 32 bytes).
  It no longer generates a fallback `sid`.
- If Torii is served behind nginx or another reverse proxy, `/v1/connect/ws`
  must preserve the websocket upgrade hop. For nginx, use
  `proxy_http_version 1.1`, `proxy_set_header Upgrade $http_upgrade`, and
  `proxy_set_header Connection "upgrade"` on the proxied websocket route.
- Keep `/v1/connect/ws` in its own exact-match location and do not rely on a
  generic `location /` or `location ^~ /v1/` proxy stanza to handle websocket
  upgrades for Connect.
- A practical smoke test for a reverse-proxied Connect deploy is:
  1. `POST /v1/connect/session` with `accept: application/json` and a
     client-generated 32-byte base64url `sid`.
  2. Reuse the returned `sid` plus `token_app` or `token_wallet` in a websocket
     upgrade request to `/v1/connect/ws?sid=<sid>&role=<role>`.
  3. Use `token_management` for `DELETE /v1/connect/session/<sid>` and token-gated
     per-session status. Use `token_relay` from the response/deep link to authenticate
     P2P Connect relay envelopes.
  4. Bind `token_relay` into wallet approval signatures with the SDK relay-auth
     hash helper before signing the approval preimage.
  5. Treat a Torii-generated application response (`400/401/...`) as proof that
     the proxy upgrade hop is working; proxy-layer `404` / missing-upgrade
     failures mean the reverse proxy still is not preserving websocket
     semantics.

See also: `crates/iroha_config/src/parameters/{user,actual}.rs` and defaults in
`crates/iroha_config/src/parameters/defaults.rs` (module `connect`).
