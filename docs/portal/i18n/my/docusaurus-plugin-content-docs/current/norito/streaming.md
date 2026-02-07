---
lang: my
direction: ltr
source: docs/portal/docs/norito/streaming.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Norito Streaming

Norito Streaming defines the wire format, control frames, and reference codec
used for live media flows across Torii and SoraNet. The canonical spec lives in
`norito_streaming.md` at the workspace root; this page distills the pieces that
operators and SDK authors need alongside the configuration touch points.

## Wire format and control plane

- **Manifests & frames.** `ManifestV1` and `PrivacyRoute*` describe the segment
  timeline, chunk descriptors, and route hints. Control frames (`KeyUpdate`,
  `ContentKeyUpdate`, and cadence feedback) live alongside the manifest so
  viewers can validate commitments before decoding.
- **Baseline codec.** `BaselineEncoder`/`BaselineDecoder` enforce monotonic
  chunk ids, timestamp arithmetic, and commitment verification. Hosts must call
  `EncodedSegment::verify_manifest` before serving viewers or relays.
- **Feature bits.** Capability negotiation advertises `streaming.feature_bits`
  (default `0b11` = baseline feedback + privacy route provider) so relays and
  clients can reject peers without matching capabilities deterministically.

## Keys, suites, and cadence

- **Identity requirements.** Streaming control frames are always signed with
  Ed25519. Dedicated keys can be supplied via
  `streaming.identity_public_key`/`streaming.identity_private_key`; otherwise
  the node identity is reused.
- **HPKE suites.** `KeyUpdate` selects the lowest common suite; suite #1 is
  mandatory (`AuthPsk`, `Kyber768`, `HKDF-SHA3-256`, `ChaCha20-Poly1305`), with
  an optional `Kyber1024` upgrade path. Suite selection is stored on the
  session and validated on every update.
- **Rotation.** Publishers emit a signed `KeyUpdate` every 64 MiB or 5 minutes.
  `key_counter` must increase strictly; regression is a hard error.
  `ContentKeyUpdate` distributes the rolling Group Content Key, wrapped under
  the negotiated HPKE suite, and gates segment decryption by ID + validity
  window.
- **Snapshots.** `StreamingSession::snapshot_state` and
  `restore_from_snapshot` persist `{session_id, key_counter, suite, sts_root,
  cadence state}` under `streaming.session_store_dir` (default
  `./storage/streaming`). Transport keys are re-derived on restore so crashes
  do not leak session secrets.

## Runtime configuration

- **Key material.** Supply dedicated keys with
  `streaming.identity_public_key`/`streaming.identity_private_key` (Ed25519
  multihash) and optional Kyber material via
  `streaming.kyber_public_key`/`streaming.kyber_secret_key`. All four must be
  present when overriding defaults; `streaming.kyber_suite` accepts
  `mlkem512|mlkem768|mlkem1024` (aliases `kyber512/768/1024`, default
  `mlkem768`).
- **Codec guardrails.** CABAC stays disabled unless the build enables it;
  bundled rANS requires `ENABLE_RANS_BUNDLES=1`. Enforce via
  `streaming.codec.{entropy_mode,bundle_width,bundle_accel}` and optional
  `streaming.codec.rans_tables_path` when supplying custom tables. Bundled
- **SoraNet routes.** `streaming.soranet.*` controls anonymous transport:
  `exit_multiaddr` (default `/dns/torii/udp/9443/quic`), `padding_budget_ms`
  (default 25 ms), `access_kind` (`authenticated` vs `read-only`), optional
  `channel_salt`, `provision_spool_dir` (default
  `./storage/streaming/soranet_routes`), `provision_spool_max_bytes` (default 0,
  unlimited), `provision_window_segments` (default 4), and
  `provision_queue_capacity` (default 256).
- **Sync gate.** `streaming.sync` toggles drift enforcement for audiovisual
  streams: `enabled`, `observe_only`, `ewma_threshold_ms`, and `hard_cap_ms`
  govern when segments are rejected for timing drift.

## Validation and fixtures

- Canonical type definitions and helpers live in
  `crates/iroha_crypto/src/streaming.rs`.
- Integration coverage exercises the HPKE handshake, content-key distribution,
  and snapshot lifecycle (`crates/iroha_crypto/tests/streaming_handshake.rs`).
  Run `cargo test -p iroha_crypto streaming_handshake` to verify the streaming
  surface locally.
- For a deep dive into layout, error handling, and future upgrades, read
  `norito_streaming.md` in the repository root.
