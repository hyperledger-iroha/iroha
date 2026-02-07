---
lang: ka
direction: ltr
source: docs/source/norito_streaming.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 143078068e8a913fef0e274fd2fe202cf235edc9e7cbb945e87c456a14edb717
source_last_modified: "2026-02-04T16:48:16.262014+00:00"
translation_last_reviewed: 2026-02-07
---

# Norito Streaming (`norito::streaming`)

This module implements the on-wire structures, helpers, and baseline codec used
by the Norito Streaming Codec (NSC). The types map one-to-one with the spec in
`norito_streaming.md` at the workspace root.

## Manifests and Control Frames

The `ManifestV1`, `PrivacyRoute*`, and control frame enums expose the
serialization surface for publishers, relays, and viewers. They are plain data
containers and must be validated by higher layers (e.g. checking signatures or
matching ticket policies) before being accepted.

## Segment Encoding and Validation

`streaming::chunk` provides utilities for chunk hashing, Merkle proof
construction, and data-availability commitments. The `streaming::codec`
submodule supplies a baseline encoder/decoder pair used in tests and as a
reference implementation:

- `BaselineEncoder::encode_segment` constructs an `EncodedSegment` consisting of
a `SegmentHeader`, `ChunkDescriptor`s, and chunk payloads. It enforces strict
monotonic chunk IDs, checks for per-frame timestamp overflow, and ensures the
resulting header’s `duration_ns` honours any explicit configuration.
- `EncodedSegment::verify_manifest` re-checks chunk commitments against a
`ManifestV1` so hosts can verify a publisher’s claims before serving viewers.
- `BaselineDecoder::decode_segment` verifies commitments and also double-checks
that frame presentation timestamps (`pts_ns`) follow the strict arithmetic
`t = timeline_start_ns + i * frame_duration_ns`, rejecting drift.

These checks mirror the requirements captured under “Segment Layout” in
`norito_streaming.md` and provide idiomatic errors (`SegmentError`, `CodecError`)
so callers can surface meaningful diagnostics.

Codec constraints enforced by the reference implementation:
- Segments are capped at `u16::MAX` frames because chunk IDs and `SegmentHeader.chunk_count`
  are encoded as `u16`. Oversized segments are rejected up-front.
- 4:2:0 chroma requires even frame dimensions. Odd-width/height streams must omit chroma or
  resample before encoding.
- Audio uses one frame per video frame; `AudioEncoderConfig.frame_samples` must match the
  rounded cadence `sample_rate * frame_duration_ns / 1_000_000_000` so timestamps remain
  deterministic.
- The optional build feature `streaming-fixed-point-dct` swaps the floating-point DCT/IDCT
  implementation for a fixed-point path to improve determinism and CPU predictability.

For future math upgrades (non-normative), see `docs/source/norito_streaming_math_notes.md`.

## Key Management

`streaming::StreamingSession` provides helpers for the control-plane handshake:
`build_key_update` signs outbound frames with Ed25519 identity keys and produces
the suite-specific ephemeral payload, while `process_remote_key_update`
validates signatures, enforces monotonic counters, and establishes session
transport keys. Kyber-based HPKE handshakes are supported via
`set_kyber_remote_public`/`set_kyber_local_secret`, which configure the static
key material required to encapsulate to viewers and decapsulate incoming
`KeyUpdate` payloads. Fingerprints use the `nsc_kyber_pk` domain with
`Sha3-256` until the workspace migrates the streaming hash functions to BLAKE3.

`streaming::StreamingKeyMaterial` wraps the node-owned Ed25519 identity key
pair alongside optional Kyber key material. `set_kyber_keys` validates the
provided byte slices, caches the `Kyber768` fingerprint, and zeroizes the secret
key when dropped. Call `install_into_session` on new `StreamingSession`
instances to pre-load the Kyber secret for HPKE decapsulation, and use
`build_key_update` to sign outbound frames without re-threading the identity
key. When operators need to pre-compute suite fingerprints for configuration,
`kyber_public_fingerprint` exposes the canonical helper used by the runtime.
Configuration files surface these knobs under the `streaming` namespace:
`streaming.kyber_public_key` and `streaming.kyber_secret_key` accept hex-encoded
Kyber key material and feed the `StreamingKeyMaterial` passed to
`StreamingHandle::with_key_material` during startup. Both fields must be
provided together; omitting them keeps HPKE disabled. Operators running with a
non-Ed25519 validator key (for example TC26 GOST) can still satisfy the Ed25519
requirement for control-plane signatures by supplying `streaming.identity_public_key`
and `streaming.identity_private_key`; these fields expect Ed25519 multihash
strings (matching the format returned by `kagami crypto`). When omitted, the
node’s main key pair is reused, preserving the previous behaviour. The
`streaming.session_store_dir` parameter selects the directory where encrypted
session snapshots land (default `./storage/streaming`). The runtime derives the
snapshot encryption key deterministically from the streaming identity, so
`StreamingHandle::load_snapshots()` can restore negotiated transport secrets and
latest GCK metadata after a restart without leaking key material to disk.
The `streaming.kyber_suite` field selects the ML-KEM profile that determines the
expected key lengths (accepting `mlkem512`, `mlkem768`, or `mlkem1024`; aliases
`kyber512/768/1024` are also recognised). The runtime currently negotiates
`mlkem768` on the wire and treats the other suites as reserved for future
upgrade stages, but validating configurations against the desired suite ensures
operators provision matching key material today.
`StreamingSession::snapshot_state` and `restore_from_snapshot` expose a compact
Norito-encoded blob containing the negotiated session ID, key counter, STS root,
cadence state, Kyber fingerprints, and latest GCK metadata so hosts can persist
the handshake and resume viewers after restarts without replaying the control
stream; transport keys are re-derived deterministically from the stored STS
root when the snapshot is restored. Snapshots are written to
`kura/store_dir/streaming_sessions/sessions.norito` and are encrypted with
ChaCha20-Poly1305 using a key derived from the node's Ed25519 identity.
- Deterministic session lifecycle — `StreamingSession::snapshot_state` persists `{session_id,
  key_counter, sts_root}` together with the negotiated suite, optional Kyber fingerprints, and the
  cadence tracker so restarts can resume without replaying earlier `KeyUpdate` frames. Restores
  invoke `StreamingSession::restore_from_snapshot`, which re-derives transport keys from the STS
  root and re-installs the stored cadence/login metadata before accepting new frames.

## Transport & Control Plane Design

The binding transport and control-plane decisions for Roadmap Milestones D2–D7
are detailed in `norito_streaming_transport_design.md`. The summary below
captures the key points so implementers can line them up with the codec spec.

### HPKE cipher suites

- Suite #1 (mandatory): `mode = AuthPsk`, `KEM = Kyber768`,
  `KDF = HKDF-SHA3-256`, `AEAD = ChaCha20-Poly1305`.
- Suite #2 (optional): `mode = AuthPsk`, `KEM = Kyber1024`,
  `KDF = HKDF-SHA3-256`, `AEAD = ChaCha20-Poly1305`.
- The PSK derives deterministically from the manifest commitment hash and is
  accompanied by a `psk_id` embedded in each `KeyUpdate` control frame. Suite
  selection resolves to the lowest common bit advertised during capability
  exchange; downgrades are detected by comparing the chosen suite ID against the
  stored value in `StreamingSession::suite`.

### Rekey cadence and persistence

- Publishers emit a new `KeyUpdate` every 64 MiB of encrypted payload or 5
  minutes. `StreamingSession::key_counter` increments per update and must never
  regress.
- Each rekey produces a fresh deterministic Kyber encapsulation (coins derived
  from session ID and counter) and rotates the Session Transport Secret (STS).
  `{session_id, key_counter, suite_id, sts_root}` are persisted via
  `StreamingSession::snapshot_state` so restarts resume without replay.
- Group content keys rotate every third HPKE update; viewers retain only the
  newest value to bound memory and gossip exposure.

### QUIC profile and capability negotiation

- During the QUIC handshake the peers exchange a Norito-encoded
  `TransportCapabilities` frame on control stream `0`. It advertises the HPKE
  suite bitmask, DATAGRAM support, maximum segment DATAGRAM size, feedback
  interval, and telemetry bucket granularity.
- Capability resolution picks the intersection of suite bitmasks, the logical
  AND of DATAGRAM support, the minimum DATAGRAM size, and the maximum feedback
  interval. The resolved tuple feeds the manifest’s
  `transport_capabilities_hash` so operators can audit deployments.
- Feature negotiation intersects the viewer’s `feature_bits` with the
  publisher’s advertised capability mask. Viewers may set bit 10 to REQUIRE a
  privacy overlay; publishers lacking bit 11 **MUST** reject the handshake with
  a protocol violation. Unknown feature bits are rejected to keep the codec surface
  deterministic.
- Node configuration: `streaming.feature_bits` in the node config controls the
  publisher’s advertised mask. The default template enables both feedback hints
  and the privacy-overlay provider bits (`0b11`), matching the required baseline (bit 0 = feedback hints, bit 1 = privacy provider). Networks that admit SM2/SM3/SM4
  transactions automatically set bit 8 during capability negotiation so viewers and relays
  can detect SM-capable manifests; binaries built without the `sm` feature clear that bit even if
  the operator-supplied mask includes it.
- Media travels via QUIC DATAGRAMs when supported; otherwise publishers fall
  back to deterministic unidirectional streams with monotonic stream IDs per
  segment window. Control messages (key updates, feedback) ride a dedicated
  bidirectional stream with the highest priority weight.

### MTU negotiation and fallbacks

- `StreamingConnection::max_datagram_size` and `StreamingConnection::datagram_enabled`
  clamp DATAGRAM payloads to the negotiated minimum and expose a runtime guard so
  hosts can pivot to the fallback path when peers disable DATAGRAM delivery.【F:crates/iroha_p2p/src/streaming/quic.rs:375】
- The capability handshake now enforces consistent `CapabilityReport`/`CapabilityAck`
  MTU values and rejects zero-length DATAGRAM requests when the transport stays in
  DATAGRAM mode; integration tests cover the negotiated limit as well as the
  DATAGRAM-off path.【F:crates/iroha_p2p/src/streaming/quic.rs:982】
- Restricted environments (enterprise firewalls, TURN relays) should enable the
  deterministic fallback by muxing chunk payloads over per-segment unidirectional
  streams or tunnelling the QUIC control plane through the existing `/p2p` TCP
  listener. Chunk framing, Norito manifest commitments, and privacy masks must be
  identical across DATAGRAM and fallback modes so viewers cannot distinguish paths.

### Congestion and FEC model

- Congestion control is fixed to BBRv1 with `(startup_gain = 2.885,
  drain_gain = 1/2.885, pacing_gain_cycle = [1.25, 0.75, 1.0, 1.0, 1.0, 1.0,
  1.0, 1.0])` and a minimum RTT floor of 5 ms.
- Viewers emit `FeedbackHint` frames every negotiated interval with
  `(loss_ewma, latency_grad, observed_rtt_ms)` computed in Q16.16 fixed-point.
  Every third hint they send a `ReceiverReport` including `parity_applied` and
  the current redundancy budget.
- Publishers compute parity with
  `parity_chunks = clamp(ceil((loss_ewma * 1.25 + 0.005) * 12), 0, 6)` in
  fixed-point arithmetic. Redundancy may only increase within a session window.
- The runtime records this parity inside `StreamingSession` snapshots so the
  `ManifestPublisher` (`iroha_core::streaming::ManifestPublisher`) can call
  `StreamingHandle::populate_manifest(peer_id, manifest, feedback_hint_frame)`
  before emitting `ControlFrame::ManifestAnnounce`, stamping the negotiated
  capability hash, cadence, and monotonic parity (when available) while falling
  back gracefully when no viewer feedback has been recorded yet. The call also
  rewrites `manifest.capabilities` with the publisher’s configured feature mask,
  ensuring bundled rANS (`FEATURE_ENTROPY_BUNDLED`) and accelerator bits stay
  embedded in the manifest regardless of which viewer triggered publication.

### Telemetry and privacy rules

- Session identifiers and manifest IDs are hashed with domain-separated BLAKE3
  keys before exporting metrics. Counters include:
  `streaming_hpke_rekeys_total{suite_id}`, `streaming_quic_datagrams_sent_total`,
  `streaming_quic_datagrams_dropped_total`,
  `streaming_fec_parity_current{bucket}`, `streaming_feedback_timeout_total`,
  and `streaming_privacy_redaction_fail_total`.
- Encode/decode/network/energy telemetry now land in Prometheus histograms and counters:
  `streaming_encode_latency_ms`, `streaming_encode_audio_jitter_ms`, `streaming_encode_audio_max_jitter_ms`, `streaming_encode_dropped_layers_total`,
  `streaming_decode_buffer_ms`, `streaming_decode_dropped_frames_total`, `streaming_decode_max_queue_ms`, `streaming_decode_av_drift_ms`, `streaming_decode_max_drift_ms`,
  `streaming_decode_max_queue_ms`, `streaming_network_rtt_ms`,
  `streaming_network_loss_percent_x100`, `streaming_network_fec_{repairs,failures}_total`,
  `streaming_network_datagram_reinjects_total`,
  `streaming_energy_encoder_mw`, and `streaming_energy_decoder_mw`. The same data
  is exported via `TelemetryEvent::{Encode,Decode,Network,Energy,AuditOutcome}` for log consumers.
- Loss EWMA buckets: `[0,0.01)`, `[0.01,0.05)`, `[0.05,0.1)`, `[0.1,0.2)`,
  `[0.2,0.4)`, `[0.4,1.0]`. Latency gradient buckets: `<-5 ms`, `[-5,-1)`,
  `[-1,1)`, `[1,5)`, `[5,10)`, `>=10`. FEC parity buckets: `0`, `1`, `2`, `3`,
  `4`, `>=5`.
- Raw feedback samples live only in encrypted session snapshots and are deleted
  within 36 hours or when the manifest expires. Exporters emit aggregates at
  most once per minute to protect low-volume tenants.
- Security events emit `TelemetryEvent::Security` through the telemetry
  channel on every HPKE rekey or GCK rotation, carrying the cumulative counters
  alongside the negotiated suite for downstream auditing.
- Routed-trace checkpoints emit `TelemetryEvent::AuditOutcome` so log consumers can
  capture `trace_id`, `slot_height`, `reviewer`, `status`, and any mitigation link
  associated with an audit window.

Example Prometheus scrape:

```bash
$ curl -s http://localhost:8080/metrics | rg '^streaming_'
streaming_encode_latency_ms_count 1
streaming_encode_latency_ms_sum 18
streaming_network_rtt_ms_sum 33
streaming_network_fec_repairs_total 4
streaming_energy_encoder_mw_sum 950
```

## Relay Reputation & Incentives

The streaming orchestrator persists the per-provider scoreboard whenever a
fetch run executes with `--orchestrator-config=…` (see
`docs/examples/sorafs_direct_mode_policy.json`). Publishers can now turn that
snapshot plus live telemetry into a deterministic “trust list” and reward
schedule with the helper script below:

```bash
scripts/norito_relay_selector.py \
  --scoreboard docs/examples/norito_relay_scoreboard_sample.json \
  --metrics docs/examples/norito_relay_metrics_sample.csv \
  --json-out artifacts/norito_relay_incentives.json
```

The script ranks relays using three signals:

1. **Scheduler weight** – the normalised score exported by the orchestrator,
   already factoring eligibility and capability checks.
2. **Reliability** – uptime and failure rates sourced from Prometheus /
   orchestrator telemetry (values in the `metrics` CSV). Repeated invalid
   stream tokens apply a quadratic penalty so abusive relays slide to the bottom.
3. **Observed usage** – delivered byte share ensures relays that actually serve
   traffic receive proportionally larger payouts, keeping incentives aligned
   with work performed.

Relays falling below the configurable `--min-score` threshold are excluded
from the reward pool. The JSON report contains the blended score and the
proportional reward share for each relay so governance can mint tokens or
credits deterministically at the end of an epoch. Operators can wire the same
output into their admission tooling to rotate allowlists without hand-curated
lists.

> **Telemetry schema** – the example JSON/CSV under
> `docs/examples/norito_relay_scoreboard_sample.json` and
> `docs/examples/norito_relay_metrics_sample.csv` documents the expected
> columns: `provider_id`, `delivered_bytes`, `uptime_ratio`, `failure_ratio`,
> `tickets_valid`, and `tickets_invalid`. Exporters should emit one row per
> relay per accounting window (typically 24h).

### Kaigi Sessions

Kaigi provides paid, real-time audio/video rooms on the Sora Nexus. Hosts
can orchestrate sessions entirely through ISIs, and every transition consumes
gas deterministically:

- `CreateKaigi` registers a call under a domain with host-provided billing
  metadata (`gas_rate_per_minute`, optional billing account, capacity limits).
- `JoinKaigi`/`LeaveKaigi` manage participant rosters while enforcing the
  configured capacity caps and host authorization.
- `EndKaigi` freezes the session and stamps the closing timestamp.
- `RecordKaigiUsage` appends metered segments so billing totals (duration and
  gas) accumulate inside the call record.
- The ledger emits `KaigiRosterSummary`, `KaigiRelayManifestUpdated`, and
  `KaigiUsageSummary` domain events whenever rosters, relay manifests, or
  aggregate usage change, letting subscribers monitor sessions without
  scraping metadata blobs.

### CLI Helpers

Operators can curate Kaigi calls directly from the CLI:

```bash
# Register a session
iroha kaigi create --domain streaming --call-name daily --host ih58... \
  --privacy-mode transparent --room-policy public \
  --relay-manifest manifests/torii.json

# Join/leave flows for participants
iroha kaigi join --domain streaming --call-name daily --participant ih58...
iroha kaigi leave --domain streaming --call-name daily --participant ih58...

# Record billable usage (milliseconds + gas)
iroha kaigi record-usage --domain streaming --call-name daily \
  --duration-ms 120000 --billed-gas 1500
```

Each subcommand feeds the standard CLI submission pipeline, so flags like
`--metadata-json` and hex proofs integrate with existing transaction handling.

Calls are materialised as deterministic entries in the hosting domain’s
metadata using the key prefix `kaigi__<name>`. Each record stores the host,
participant list, accumulated usage, and arbitrary metadata for downstream
applications. Because the records live alongside canonical `DomainEvent`
metadata updates, downstream indexers can subscribe to standard metadata events
to monitor call creation, join/leave churn, and usage reporting.
Hosts choose whether exit relays enforce viewer authentication via
`--room-policy` (`public` maps to `stream.kaigi.public`; `authenticated` maps to
`stream.kaigi.authenticated`). When omitted, rooms default to requiring
authentication.
Hosts may optionally advertise a `scheduled_start_ms`, which is preserved
alongside the actual `created_at_ms` timestamp recorded at admission time.
Sessions can opt into `privacy_mode = ZkRosterV1`, replacing the on-ledger
participant list with commitment/nullifier sets and attaching an optional
`relay_manifest` (presently stored as a compact Norito JSON blob) that
describes the onion-routing path for control/data channels. Transparent
deployments keep the classic roster while new privacy fields remain empty.

## Tests

Unit tests under `streaming::codec::tests` cover segment verification, manifest
mismatch detection, and the timestamp invariants added above. Dedicated golden
fixtures keep the baseline DCT/quantisation pipeline and the single-block
entropy stream stable, while proptest scenarios exercise chunk Merkle proofs and
storage commitments. The suite also asserts that malformed run-length streams
surface `CodecError::RleOverflow` or `CodecError::MissingEndOfBlock`, providing
clear diagnostics for corrupted payloads. These checks serve as a regression
suite and as guidance for integrators building their own encoders.

## Operator SLA Obligations

Streaming deployments must hold the same deterministic guarantees the codec and
transport were designed for. Operators should track the following commitments and
wire them into their alerting dashboards alongside the metrics exported in
`streaming_*`:

- **Cadence budgets.** Maintain end-to-end segment cadence ≤ 33 ms and encode latency
  ≤ 20 ms at the `streaming_encode_latency_ms_{sum,count}` histogram P99. The network
  path must keep `streaming_network_rtt_ms_sum / count` under 75 ms and sustain
  `streaming_network_fec_repairs_total / streaming_network_segments_total ≤ 0.05`
  outside of declared maintenance windows. These bounds ensure receivers continue
  to meet the ±10 ms A/V sync tolerance enforced during validation.
- **Session store retention.** Persist session manifests, relay manifests, and usage
  summaries for at least 30 days (or the jurisdictional maximum if stricter) so that
  billing, dispute resolution, and privacy audits remain reproducible. Stale
  manifests should be compacted weekly, but never before the retention window
  elapses.
- **Relay uptime.** Each relay advertised in `relay_manifest` must achieve 99.5 %+
  availability per rolling 30-day window with no more than three consecutive minutes
  of downtime. Operators should alert if the Prometheus counters backing
  `streaming_network_relay_unavailable_total` increase more than once per hour.

Publish these commitments in local runbooks and customer-facing contracts; any
breach should trigger the incident workflow described in `docs/source/telemetry.md`.

### Cross-language conformance vectors

Milestone D7’s cross-language harness ships in the repository and is ready for SDK
teams to consume:

- Canonical Rust integration tests live under
  `integration_tests/tests/norito_streaming_{end_to_end,feedback,fec,negative}.rs`
  with shared helpers in `integration_tests/tests/streaming/mod.rs`. The tests run the
  full publisher↔viewer handshake, parity recovery, and error paths against a
  deterministic schedule.
- The fixtures driving those tests are published in
  `integration_tests/fixtures/norito_streaming/rans/baseline.json` and the bundled-profile
  companion `integration_tests/fixtures/norito_streaming/rans/bundled.json`, alongside the
  conformance bundle in `docs/assets/nsc/conformance/`. SDK harnesses should load the
  JSON vectors and reproduce the digests listed under `docs/assets/nsc/conformance/entropy.json`.
  Regression tests `baseline_snapshot_matches_golden_fixture` and
  `bundled_snapshot_matches_golden_fixture` guard those JSON snapshots so any drift
  immediately shows up in CI before SDKs pick up stale fixtures.
- JavaScript integration guidance already demonstrates the Torii streaming workflow in
  `docs/source/sdk/js/quickstart.md` (see “Torii Queries & Streaming”). Other SDKs should
  replicate the same fixture round-trips and report parity/entropy statistics alongside
  the Rust harness when publishing their conformance results.

The round-trip (`integration_tests/tests/norito_streaming_roundtrip.rs`) and RS12_10
parity harness (`integration_tests/tests/norito_streaming_fec.rs`) now iterate over both
the baseline CABAC fixture and the bundled `EntropyMode::RansBundled` vector so entropy
changes cannot regress manifest verification or parity recovery.

To reuse the harness from another language, mirror the manifest and frame sequence emitted
by `integration_tests/tests/streaming/mod.rs::baseline_test_vector_with_frames` and verify
the decoded output against the hashes in the conformance bundle. CI jobs can invoke
`cargo test -p integration_tests --tests norito_streaming_*` as the reference oracle.

## Implementation Notes

- The baseline encoder continues to compress luma-only frames, but RD bundles now stash deterministic 4:2:0 chroma sidecars (no neutral fills) so decoded Y4M outputs and PSNR-YUV calculations match the source. Full chroma compression/HDR variants and SIMD acceleration are planned for Milestone D8; keep production traffic on the baseline profile until those land.
- A perceptual RDO schedule is available (`RdoMode::Perceptual`) for bundled entropy; it softens lambda at mid/high Q to bias toward SSIM-like structure preservation. The RD harness can drive the bundled encoder with explicit quantizers via `--quantizer` (repeatable for sweeps) or request a bitrate ladder with `--target-bitrate-mbps` plus the tiny-clip preset (`--tiny-clip-preset`) to keep 16–32 px fixtures from paying oversized headers. JSON outputs now enumerate every quantizer run and record bundled metrics alongside baseline values when `ENABLE_RANS_BUNDLES=1` is set.
- Congestion-control feedback (`FeedbackHint`/`ReceiverReport`) is scheduled to be retrofitted during Milestone D3. Until then, telemetry counters track the placeholder behaviour described above.
- The Norito audio helper now defaults to the native low-delay codec (block-adaptive delta quantisation) and keeps libopus as an explicit fallback. Deployments that force libopus retain the 64 kbps mono / 96 kbps stereo presets, while ambisonics continues to leverage the deterministic codec until multistream support lands.
- Segment headers and manifests include an `audio_summary` (sample rate, frame samples, cadence, FEC level, layout). Validators enforce ±10 ms A/V sync using this summary when checking timestamps in the pipeline.
- Decoder/RD harness: `cargo xtask streaming-entropy-bench` now emits decoded Y4M clips, PSNR, and Norito `SegmentBundle` artefacts (with optional chroma sidecars) when `--y4m-in/--y4m-out/--chunk-out` are supplied, and `cargo xtask streaming-decode --bundle <path> --y4m-out <path> [--psnr-ref <y4m>] [--psnr-mode y|yuv]` rehydrates bundles for RD tooling. The rANS comparison harness (`benchmarks/nsc/rans_compare.py`) records PSNR, SSIM, optional VMAF, bundled chunk sizes, bitrate ladder selections, and the `SVT_MIN_DIMENSION` guard in `report.json`, and now publishes `norito_summary`/`norito_per_clip_summary` blocks so dashboards can read per-clip baseline vs bundled metrics (with skip flags) without scraping runner logs.【xtask/src/streaming_bench.rs:1】【benchmarks/nsc/rans_compare.py:1】

### Bundled rANS entropy mode

`EntropyMode::RansBundled` unlocks the NSC-55 bundled rANS profile. The encoder and
decoder already ship in `norito::streaming`; operators move traffic to the new profile
by flipping the runtime toggles and proving that counterparts negotiated the matching
capability bit.

#### Configuration

Set the codec stanza in `iroha_config` (or the matching CLI flags) so every node
advertises the same bundle profile:

```toml
[streaming.codec]
cabac_mode = "disabled"                # only change after ENABLE_CABAC builds pass legal review
trellis_blocks = []                    # keep empty until the claim-avoidance profile ships
rans_tables_path = "codec/rans/tables/rans_seed0.toml"
entropy_mode = "rans_bundled"          # allowed values: rans | rans_bundled
bundle_width = 2                       # bundled profile requires 2..=3 (per bundled table set)
bundle_accel = "cpu_simd"              # allowed values: none | cpu_simd | gpu (requires bundled entropy)
bundle_prefetch_distance = 0           # optional: prefetch depth for bundle ANS encoder (0 disables)
```

Bundled mode is available only when the binaries are built with
`ENABLE_RANS_BUNDLES=1` (`norito::streaming::BUNDLED_RANS_BUILD_AVAILABLE` evaluates to
`true`). Builds lacking the flag will reject streaming configs because bundled entropy
is mandatory. The runtime defaults to `entropy_mode = "rans_bundled"` even if the stanza
bundled profile with `bundle_width >= 2`.
Binaries ship bundled rANS tables for widths 2 and 3; wider values are rejected
at config-parse time to avoid running against unpinned tables.

Bundled telemetry now exposes RDO metadata (trellis-free DP with energy buckets
`[0, 64, 256, 1024, 4096+]` and an optional int8 neural selector) plus per-context
symbol histograms. Feed the resulting `bundled_telemetry.json` into
`cargo xtask streaming-context-remap --input <path> --top <n>` to produce deterministic
remap tables when pruning low-volume contexts prior to enabling advanced entropy
profiles on mixed clusters.
Zero-only runs are now grouped per remapped context into `SignificanceRle` bundles;
the remap summary (escape context defaulting to `0xFFFF`, remapped/dropped counts)
rides inside bundled telemetry and can be reloaded via
`load_bundle_context_remap_from_json` to align encoder/decoder table refreshes.
When built with the `streaming-neural-filter` feature, decoders run a deterministic
int8 3×3 neural filter over reconstructed luma before cropping/prediction to keep
texture retention deterministic across architectures.

The bundled encoder now ships a four-lane rANS backend for `bundle_accel = "cpu_simd"`
with runtime AVX2/NEON detection and a deterministic fallback to the scalar pipeline.
The ANS telemetry stream advertises the SIMD format with the `BR4\x01` magic and records
the effective `acceleration` plus `prefetch_distance` so dashboard consumers can tell
which path produced the artefact. Operators may set `bundle_prefetch_distance > 0` to
issue prefetch hints while encoding; the value is purely a performance hint and does
not alter the emitted bitstream.

The parser in `crates/iroha_config/src/parameters/user.rs` rejects invalid
combinations—bundled mode clamps the width to `2..=4` and `bundle_accel` only accepts
selections are rejected outright. `rans_tables_path` must point at the canonical
`SignedRansTablesV1` artefact (`codec/rans/tables/rans_seed0.toml` by default); refresh
the tables with `python3 tools/rans/gen_tables.py --bundle-width 4 --seed <seed> --output codec/rans/tables/rans_seed<seed>.toml --verify`
when governance publishes a new seed, and use `cargo xtask verify-tables --tables codec/rans/tables/<file>.toml`
to audit existing artefacts. `StreamingHandle::apply_codec_config` loads the file at
start-up, records the checksum, and threads the bundle width/acceleration preference
into the runtime.

#### Capability negotiation and manifests

When bundled mode is enabled the runtime automatically:

1. Sets `CapabilityFlags::FEATURE_ENTROPY_BUNDLED` on the viewer side so peers know
   the decoder can consume bundled manifests.
2. Switches manifest/segment headers to `entropy_mode = RansBundled` and injects the
   deterministic `entropy_tables_checksum` next to the chunk descriptors.
3. Rejects attempts to publish bundled manifests from nodes that were not configured
   for bundled mode (`StreamingProcessError::ManifestEntropyModeUnsupported`).
4. Drops viewers that fail to negotiate the bundled capability bit during the control
   plane handshake (`StreamingProcessError::ManifestEntropyModeNotNegotiated`).

This guardrail matches the test coverage in `crates/iroha_core/src/streaming.rs` (see
`bundled_manifest_checksum_mismatch_rejected`,
`viewer_requires_negotiated_bundled_capability`, and
`bundled_entropy_requires_viewer_support`). Run

```bash
cargo test -p iroha_core bundled_manifest_checksum_mismatch_rejected \
  viewer_requires_negotiated_bundled_capability \
  bundled_entropy_requires_viewer_support
```

to prove the capability checks and manifest hashing still hold when enabling the new
profile. The manifest helpers exposed through `StreamingHandle::bundle_tables_checksum`
and `normalize_viewer_feature_bits` make it straightforward for SDKs to surface the
same checks when they start emitting bundled payloads.

#### Deployment evidence helper

Use `cargo xtask streaming-bundle-check --config <path>` to capture the codec
state and bundled-table checksum for any node configuration. The helper parses
`extends` chains, loads the configured `SignedRansTablesV1` artefact, and emits
JSON describing the runtime feature bits, entropy mode, bundle width/accel
preferences, and the canonical checksum derived from the manifest:

```bash
cargo xtask streaming-bundle-check \
  --config configs/soranexus/nexus/config.toml \
  --json-out artifacts/streaming_bundle/nexus.json
```

Sample output (edited for brevity):

```json
{
  "config": "configs/soranexus/nexus/config.toml",
  "feature_bits": 3,
  "bundle_required": true,
  "entropy_mode": "rans_bundled",
  "bundle_width": 2,
  "bundle_accel": "cpu_simd",
  "gpu_build_available": false,
  "bundle_accel_allowed": true,
  "tables": {
    "path": "codec/rans/tables/rans_seed0.toml",
    "precision_bits": 12,
    "checksum": "6f3568c43f9c71d1605f10cb43a4cf08bda413a766b87f61ab263b35b0907c2c"
  }
}
```

Attach the JSON artefact to rollout tickets and operator runbooks so mixed-cluster
deployments can prove the negotiated `entropy_mode` and checksum. During rollback
rehearsals, rerun the helper with the `--tables` override pointing at the staged
manifest to verify checksum parity before enabling the bundled capability flag.

#### Migration & rollback runbook

Operators enable bundled mode in four repeatable steps:

1. **Pre-flight.** Build with `ENABLE_RANS_BUNDLES=1` (bundled builds now default to
   `entropy_mode = "rans_bundled"`), commit the shared `[streaming.codec]` stanza, and
   archive the `streaming-bundle-check` JSON (one artefact per config) under
   explicitly set `entropy_mode = "rans"`, `bundle_width = 1`, and `bundle_accel = "none"`
   before restarting the nodes.
2. **Canary handshake.** Flip the config for a single publisher/viewer pair,
   restart both nodes, and capture the `CapabilityAck` trace showing
   `FEATURE_ENTROPY_BUNDLED` plus the acceleration bit (`cpu_simd` = bit 13,
   `gpu` = bit 14). Run
   `cargo test -p iroha_core bundled_manifest_checksum_mismatch_rejected \
   viewer_requires_negotiated_bundled_capability \
   publisher_rejects_missing_bundle_acceleration_support`
   so the promotion has deterministic evidence. Integration tests
   `bundled_manifest_handles_mixed_viewer_populations` and
   `bundled_gpu_handle_rejects_cpu_viewers`
   (`integration_tests/tests/streaming/mod.rs`) keep the multi-viewer normalization story
   honest—run them when changing capability bits to ensure CPU-only and GPU-only publishers still
   treat each viewer independently.
3. **Fleet rollout.** Roll the config through the remaining nodes, watching the
   `bundle_tables_checksum` metric and `streaming_encode_*` / `streaming_decode_*`
   counters for regressions. Update the operator runbook with the selected
   `bundle_accel` mode so future rollbacks know which capability mask to restore.
   (`entropy_mode = "rans"`, `bundle_width = 1`, `bundle_accel = "none"`),
   restart it, and confirm that manifests clear the bundled capability bit and
   omit `entropy_tables_checksum`. Capture a fresh handshake trace plus a
   `streaming-bundle-check` JSON (`bundle_required = false`) before re-promoting.

This flow mirrors Appendix I of the main Norito Streaming specification and keeps
bundle enablement deterministic across clusters.

#### GPU acceleration determinism & testing plan

Roadmap NSC‑55/Task 3d adds optional Metal/CUDA kernels on top of bundled rANS. Before
`bundle_accel = "gpu"` may roll beyond experiments the following guardrails and
evidence MUST be satisfied.

- **Build & negotiation guardrails**
  - GPU kernels only build when `ENABLE_RANS_BUNDLES=1` and the target backend flag
    (`codec-gpu-metal` / `codec-gpu-cuda`) are present. Community builds refuse
    `bundle_accel = "gpu"` during config parsing and default to `bundle_accel = "none"`.
  - `cargo xtask streaming-bundle-check` emits `gpu_build_available` and
    `bundle_accel_allowed` so rollout artefacts capture whether the binary may legally
    advertise the GPU capability bit; configs on CPU-only builds now fail during
    parsing/runtime config loading when they request `bundle_accel = "gpu"`.
  - Publishers advertising the GPU backend set `CapabilityFlags::FEATURE_BUNDLE_ACCEL_GPU`
    (bit 14). Viewers lacking that bit in `CapabilityAck` are rejected up-front
    (`StreamingProcessError::ManifestAccelerationNotNegotiated`) and the runtime falls
    back to bit 13 (`cpu_simd`) or clears both bits when the backend is disabled.
  - `cargo xtask streaming-bundle-check` plus the streaming scoreboard snapshots persist
    `bundle_accel`, the selected `NeuralBundle` id, and the Metal/CUDA shader hashes so
    reviewers can confirm which binaries were signed for a rollout. Manifests that omit
    the matching hash MUST fail validation once the GPU harness lands.

- **Deterministic microkernel requirements**
  - **DP RDO:** GPU kernels consume the same fixed-point representation (`Q15`
    coefficients, manifest-provided `dp_energy_table`, canonical `entropy_seed`) as the
    scalar path. Warp reductions accumulate in deterministic integer order (no FP atomics
    or nondeterministic shuffles). Inputs/outputs are captured in a `GpuDpVector`
    fixture so the CPU reference (`BaselineBundler::dp_cost`) can replay the decision.
  - Regression tests `bundle_stream_recorder_counts_zero_runs_and_tokens` and
    `bundle_stream_recorder_flushes_zero_only_blocks`
    (`crates/norito/src/streaming.rs`) exercise the DP/BundledStats instrumentation directly, so we
    know zero-run counters, bundle flush reasoning, and token emission stay deterministic even
    before the GPU kernels land.
  - **Neural predictor:** The int8 2×32 network keeps the weights/bias vectors and
    activation scales embedded in `NeuralBundle`. GPU execution quantizes activations,
    accumulates in 32‑bit integers, and clamps outputs exactly like
    `NeuralPredictor::forward`. Shader/PTX hashes stored in the manifest MUST match
    the binaries loaded on-device.
  - Any mismatch between GPU and CPU outputs increments
    `streaming_gpu_kernel_failures_total`, logs the offending vector, and forces the
    runtime to retry on the scalar code path while downgrading the advertised capability
    bit at the next handshake.

- **Vector & harness coverage**
  - Canonical fixtures live under `fixtures/norito/streaming/gpu/` (separate DP and
    predictor packs). Each pack stores serialized inputs/outputs and a `sha256` manifest
    so CI detects drift. Existing CPU-side regression tests
    (`bundle_stream_recorder_counts_zero_runs_and_tokens`,
    `bundle_stream_recorder_flushes_zero_only_blocks`) anchor the DP statistics, and the GPU
    compression layer now exposes parity tests
    (`gpu_encode_matches_cpu_when_available`, `gpu_decode_matches_cpu_when_available`) so Metal hosts
    can prove encode/decode equivalence ahead of the dedicated DP/predictor fixtures.
  - CI job `ci/check_norito_gpu_kernels.sh` executes those tests on macOS (Metal) and a
    Linux CUDA runner, publishing logs plus
    `artifacts/norito/gpu_kernels/<stamp>/summary.json` for governance review.
  - Integration suites add mixed-cluster cases proving that viewers without the GPU bit
    reject GPU-only manifests and that rollbacks clear the bit alongside the manifest
    hashes.

- **Telemetry & rollout evidence**
  - `StreamingHandle` records `streaming_gpu_predictor_latency_ms`,
    `streaming_gpu_dp_energy_ms`, and `streaming_gpu_kernel_failures_total` (labelled by
    device_class/chip_family/gpu_kind) so operators can monitor a 24‑hour burn-in
    window for each GPU rollout.
  - Scoreboard exports include the resolved backend (`none`/`cpu_simd`/`gpu`), the
    `NeuralBundle` kernel hashes, and latency percentiles. The relay-selector tooling
    rejects captures whose hashes diverge from the approved list.
  - Governance binders attach (1) the GPU CI bundle, (2) Grafana snapshots covering the
    metrics above, and (3) annotated `CapabilityAck` traces showing bit 14 toggling on
    and off with `bundle_accel`.

- **Rollback & fallback**
  - Rollbacks follow the existing playbook: set `bundle_accel = "cpu_simd"` (or `none`),
    restart, rerun `cargo xtask streaming-bundle-check`, and capture a capability trace
    showing bit 14 cleared. The GPU CI bundle for the rollback proves the scalar tests
    still pass with the GPU backend disabled.
  - If telemetry cannot be kept green, document the incident ID in the scoreboard and
    remain on the scalar backend until the GPU harness passes again. Partial enablement
    (mixed GPU/non-GPU publishers) is forbidden because the capability bits are
    cluster-wide.

Until every requirement above is satisfied, GPU acceleration remains experimental even
if the kernels exist in the tree.
