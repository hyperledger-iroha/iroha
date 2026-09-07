# Kaigi over SoraNet

The current relay rejects every Kaigi exit route before opening a catalog or
forwarding traffic. The local proxy has a separately tested spool bridge, but
that bridge does not establish end-to-end SoraNet transport or anonymity.
Completing the authenticated route-open and durable revocation boundaries is
an outstanding first-release outcome.

## Exit relay behaviour
- Token-bearing Kaigi filesystem routing is disabled in V1. Core rejects every
  publication before enqueue or filesystem I/O, relay configuration rejects
  `kaigi_stream.spool_dir`, and catalog admission independently rejects every
  record. There is no static/read-only exception.
- The `RouteOpenFrame` stream tag `0x02` selects the Kaigi exit path. Its second
  byte is reserved and must be zero; it is not an authentication assertion.
  The relay records the configured public GAR category for diagnostics, then
  returns `FilesystemPublicationDisabled`. A missing configured route returns
  `StreamDisabled`. Neither read-only nor authenticated records are admitted.
- The disabled adapter code derives room identifiers by BLAKE3-blinding the
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
- Omit the retired `[streaming.soranet]` node table and relay
  `kaigi_stream.spool_dir`. Either configuration is a startup error in V1.
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
