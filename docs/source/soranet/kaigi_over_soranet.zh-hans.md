---
lang: zh-hans
direction: ltr
source: docs/source/soranet/kaigi_over_soranet.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6c5c51d7ae8dd9bf9472c7fe758a1d8ffa19a99a38f7a204e7d1b22cc1d0b797
source_last_modified: "2025-12-29T18:16:36.188716+00:00"
translation_last_reviewed: 2026-02-07
---

# Kaigi over SoraNet

Kaigi interactive traffic now rides the SoraNet anonymity transport end to
end. Relays honour `kaigi-stream` exit routes, derive blinded room identifiers,
and forward traffic to Kaigi hubs while preserving the same GAR and compliance
guardrails used for Norito streaming.

## Exit relay behaviour
- Routes are sourced from `exit-<relay_id>/kaigi-stream/*.norito` catalogs,
  carrying `PrivacyRouteUpdate` records tagged with `SoranetStreamTag::Kaigi`.
- The `RouteOpenFrame` stream tag `0x02` selects the Kaigi exit path; the auth
  flag maps to `stream.kaigi.public` or `stream.kaigi.authenticated` GAR
  categories. Relays enforce the `SoranetAccessKind` from the route and reject
  unauthenticated viewers when required.
- Room identifiers are BLAKE3-blinded from the `{channel_id, route_id,
  stream_id}` tuple so observers cannot correlate Kaigi room metadata with the
  underlying channel.
- Exit adapters convert Kaigi multiaddrs to WebSocket targets; unsupported
  multiaddrs fall back to the configured hub URL so circuits stay usable while
  route catalogs converge. Compliance logging records the channel, route,
  stream, room id, GAR category, and exit target for every open.

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
- Refresh Kaigi route catalogs alongside Norito routes; rotation/expiry respects
  the shared `route_refresh_secs` cadence.
- Validate proxy bridging locally with
  `cargo test -p sorafs_orchestrator kaigi_bridge_streams_spool_payload_with_policy`
  to exercise the spool, cache tags, and policy acknowledgement.
- Monitor exit telemetry for Kaigi via proxy transport events and relay
  compliance logs; GAR categories default to
  `stream.kaigi.public`/`stream.kaigi.authenticated`.
