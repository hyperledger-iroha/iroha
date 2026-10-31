# Kaigi over SoraNet

The relay rejects every Kaigi exit route before opening a catalog or forwarding
traffic, and no client implements the sending side. The local proxy has a
separately tested spool bridge, but it never touches SoraNet: it streams a local
file, and its `authenticated` room policy is only a label. Authenticated routing
is an outstanding first-release outcome that needs the decisions listed under
[Outstanding work](#outstanding-work).

## Exit relay behaviour
- Token-bearing Kaigi filesystem routing is disabled in V1. Relay configuration
  rejects `kaigi_stream.spool_dir` at startup, and a route-open fails even when a
  route is configured. There is no static/read-only exception.
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
  `cargo test -p sorafs_orchestrator --features local-quic-proxy kaigi_bridge_streams_spool_payload_with_policy`
  to exercise the spool, cache tags, and policy acknowledgement.
- Treat any Kaigi exit-routing activity in V1 as a configuration error; no GAR
  category is active while token-bearing filesystem routes are disabled.

## Outstanding work

Nothing consumes `KaigiRelayHop.hpke_public_key` yet: no client seals to it and
no relay opens with it. The disabled adapter (`handle_kaigi_stream`) is compiled
out and no longer builds against its surroundings. Enabling routing needs:

1. **A trust model (owner decision).** Who issues a viewer's route credential, how
   much an exit relay may link across viewers, what replaces a media "segment" for
   a live call, and who operates the hub the exit dials.
2. A route-open proof type bound to relay, channel, route, stream and validity
   window, carried in a `RouteOpenFrame` that is a fixed 34 bytes today. The VPN
   helper ticket (`VpnHelperTicketV1`) is the closest template: issuer-signed,
   relay-bound and replay-reserved.
3. Session binding through the circuit's handshake transcript hash, a verifier and
   its configuration, and a replay ledger on `PersistentReplayLedger`.
4. A revocation decision. Short-lived credentials alone are small; durable
   tombstones with a publisher are large.
5. Reviving the exit adapter, then building the hub receiver and the client dial
   and sender, neither of which exists.
