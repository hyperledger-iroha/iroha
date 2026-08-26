# Kaigi over SoraNet

Kaigi interactive traffic now rides the SoraNet anonymity transport end to
end. Relays honour `kaigi-stream` exit routes, derive blinded room identifiers,
and forward traffic to Kaigi hubs while preserving the same GAR and compliance
guardrails used for Norito streaming.

## Exit relay behaviour
- Token-bearing Kaigi filesystem routing is disabled in V1. Core rejects every
  publication before enqueue or filesystem I/O, relay configuration rejects
  `kaigi_stream.spool_dir`, and catalog admission independently rejects every
  record. There is no static/read-only exception.
- The `RouteOpenFrame` stream tag `0x02` selects the Kaigi exit path. Its second
  byte is reserved and must be zero; it is not an authentication assertion.
  Relays currently admit only `SoranetAccessKind::ReadOnly` routes and map them
  to the `stream.kaigi.public` GAR category. Authenticated records fail closed
  until a viewer credential is cryptographically bound to route opening.
- The reserved adapter derives room identifiers by BLAKE3-blinding the
  `{channel_id, route_id, stream_id}` tuple after a future route has passed the
  missing proof and revocation boundaries.
- `exit_multiaddr` is retained only as signed diagnostic metadata. Exit adapters
  dial the operator-configured exact canonical `wss://` `hub_ws_url` and never
  convert, redirect, or fall back to any catalog address. Configuration rejects
  plaintext WebSockets, userinfo, queries, fragments, authority escapes, ambiguous or
  non-canonical hosts/ports, and zero ports. The exit token is attached only
  after the exact configured TLS WebSocket handshake succeeds. Compliance
  logging records the channel, route, stream, room id, GAR category, diagnostic
  multiaddr, and configured exit target for every future admitted open.

## Local proxy bridge (browser/SDK)
- `sorafs_cli` exposes Kaigi payloads to browsers/SDKs via
  `--local-proxy-kaigi-spool <DIR>` and the optional
  `--local-proxy-kaigi-policy public|authenticated` override. Spool layout
  mirrors relay catalogs: `DIR/kaigi/<target>.norito` is streamed after a
  `room-policy=<...>` acknowledgement.
- Browser manifests advertise Kaigi room policy hints
  (`kaigi`, `kaigi.room_policy.<label>`) so clients can align with operator
  expectations; cache tags are attached when a guard cache key is present,
  matching Norito/CAR behaviour.

## Operator checklist
- Leave `streaming.soranet.enabled = false` and omit relay
  `kaigi_stream.spool_dir`. Explicit producer enablement or relay spool
  configuration is a startup error.
- Re-enablement requires a replay-protected RouteOpen proof binding viewer
  authority, selected route, and authoritative segment plus a durable
  unpublish/tombstone lifecycle. The intended custody contract is a direct
  effective-UID-owned mode-`0700` directory chain with no named symlink
  component and one direct single-link mode-`0600` channel-bound token file,
  published through private write-sync-atomic-replace-directory-sync steps.
- Validate proxy bridging locally with
  `cargo test -p sorafs_orchestrator kaigi_bridge_streams_spool_payload_with_policy`
  to exercise the spool, cache tags, and policy acknowledgement.
- Treat any Kaigi exit-routing activity in V1 as a configuration error; no GAR
  category is active while token-bearing filesystem routes are disabled.
