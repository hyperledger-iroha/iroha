---
lang: ba
direction: ltr
source: docs/source/taikai_ingest_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 06f9c907a45b8dd1ddaed39bf0960e0e7fb4da0c8d68ee578e1fd30a0c8570ec
source_last_modified: "2026-01-22T14:35:37.645927+00:00"
translation_last_reviewed: 2026-02-07
---

# Taikai Ingest & Encoder Plan (SN13-A)

_Status: Completed — Media Platform WG / DA Program / Networking TL_  
Related roadmap item: **SNNet-13A (Ingest/encoder pipeline)**

This note captures the engineering and operations plan for the Taikai ingest
surface and documents the shipped ingest edge harness. It explains how SRT/RTMP
ingest, ladder construction, LL-CMAF segmenting, deterministic CAR/TSE bonding,
and telemetry evidence are wired end to end now that SN13-A is closed. The plan
complements the canonical envelope spec (`docs/source/taikai_segment_envelope.md`)
and the data-model definitions in `crates/iroha_data_model/src/taikai.rs`.

## 1. Objectives

1. Accept SRT/RTMP inputs from publishers, normalize them into a deterministic
   mezzanine, and emit CMAF ladder rungs for LL and standard latency viewers.
2. Seal each CMAF fragment into a CARv2 archive plus `TaikaiSegmentEnvelopeV1`
   so gateways, SDKs, and DA tooling can prove availability without reprocessing
   video assets.
3. Provide CLI/SDK helpers (`iroha app taikai bundle`, `sorafs_cli taikai bundle`,
   `iroha app taikai ingest --watch`)
   that automate drift correction, metadata emission, and Norito encoding.
4. Expose ingest/encoder telemetry (latency, live-edge drift, rebuffer
   forecasts) via the `taikai_viewer`/`taikai_cache` Grafana bundles and alert
   packs referenced in the roadmap.
5. Publish fixtures and deterministic tests (`integration_tests/tests/taikai_da.rs`)
   proving the ingest path is replayable end to end.

## 2. Architecture Overview

```
publishers -> SRT/RTMP edge -> mezzanine normalizer (color space, audio layout)
            -> ladder builder (AV1/HEVC + AAC/Opus profiles)
            -> LL/STD CMAF segmenter (200 ms / 2 s targets)
            -> ingest writer (Torii /v1/da/ingest)
            -> CARv2 builder + Taikai envelope + indexes
            -> SoraNS anchor + DA replication + viewer caches
```

Each stage emits deterministic manifests:

- **Edge receivers.** SRT/RTMP listeners authenticate publishers (GAR policy),
  enrich streams with clock samples, and enqueue raw TS data into the mezzanine
  queue. Receiver config lives in `configs/taikai_ingest/*.toml`.
- **Normalizer.** Converts inputs to a fixed color space (BT.709), audio layout
  (stereo, 48 kHz), and frame cadence. Drift between sender PCR/PTS and ingest
  wall clock is tracked in `drift_monitor.json`.
- **Ladder builder.** Produces at least the following rung presets:
  `2160p/25 Mbps`, `1440p/12 Mbps`, `1080p/8 Mbps`, `720p/4 Mbps`, `480p/2 Mbps`,
  plus `audio-only` (128 kbps Opus). Profiles are recorded in
  `fixtures/taikai/ladder_presets.json`.
- **Segmenter.** Writes CMAF fragments for LL (0.2–0.5 s) and standard latency
  (2–6 s) streams. Sequence numbers and wall-clock stamps feed directly into
  `TaikaiSegmentEnvelopeV1`.
- **Ingest writer.** Calls `/v1/da/ingest` with deterministic metadata,
  verifying replication policy tags (`da.taikai.live`, `da.taikai.archive`) per
  `docs/source/da/replication_policy.md`.
- **Bundler.** Uses `sorafs_car::taikai::bundle_segment` (invoked via
  `iroha app taikai bundle`) to produce CARv2 archives, envelope `.to` files, and
  index manifests. The CLI now ships `iroha app taikai ingest --watch`, which watches
  CMAF fragment directories, derives manifest/storage-ticket digests, emits
  ladder metadata (backed by `fixtures/taikai/ladder_presets.json`), writes
  artifacts under `artifacts/taikai/ingest_run_<timestamp>/`, and generates
  canonical `/v1/da/ingest` requests (with optional live publishing).

## 3. Component Details

### 3.1 Edge Receivers (SRT/RTMP)

- Configuration: `configs/taikai_ingest/srt.toml`, `.../rtmp.toml`.
- Auth: tokens minted by GAR v2 (`docs/source/taikai_policy.md`), sent via
  `Sora-Auth` headers or RTMP `connect` parameters.
- Telemetry: `taikai_ingest_edge_total{protocol,outcome}` increments on connect/
  disconnect events. Runs under `sorafs_orchestrator::runtime::taikai`.
- Clock sampling: embed sender PCR every 2 s and store in `drift_monitor.bin`.

### 3.2 Mezzanine Normalizer

- Uses `ffmpeg`/`rust-av` graph (TBD in `tools/taikai-ingest` crate) to enforce:
  - BT.709 color space, progressive frames.
  - Audio converted to 48 kHz PCM before Opus/AAC encoding.
  - GOP cadence 2 s (LL path uses mini-GOP).
- Drops invalid timestamps and records them in
  `taikai_ingest_invalid_pts_total`.

### 3.3 CMAF Ladder Builder & Segmenter

- Ladder definitions stored in `fixtures/taikai/ladder_presets.json`.
- Encoders expose deterministic settings: CBR with `vbv-maxrate = target`,
  `vbv-bufsize = 2× target`, consistent `keyint`.
- Segmenter writes CMAF fragments into `artifacts/taikai/<event>/<stream>/`.
- Drift monitor:
  - Compare `segment_start_pts` against `wallclock_unix_ms`.
  - Write `drift_ms` into `TaikaiInstrumentation` (`taikai.instrumentation.live_edge_drift_ms`).
  - Trigger alert `TaikaiLiveEdgeDrift` when `|drift| > 1500 ms`.

### 3.4 CAR/TSE Bundler

- CLI: `iroha app taikai bundle` / `sorafs_cli taikai bundle` (already available).
- Outputs:
  - `.car` archive (CARv2) with deterministic ordering.
  - `.to` Norito envelope referencing CAR digest and storage ticket.
  - `indexes.json` (time + CID keys).
  - `ingest_metadata.json` for `/v1/da/ingest`.
- The standalone `taikai_car` CLI (SN13-B) now reuses `BundleRequest` to
  regenerate envelopes from stored fragments and supports `--summary-out` for
  automation bundles.

### 3.5 Drift & Health Monitoring

- Metrics:
  - `taikai_ingest_segment_latency_ms` (encoder→ingest).
  - `taikai_ingest_live_edge_drift_ms`.
  - `taikai_ingest_segment_errors_total{reason}`.
- Logs: `telemetry::taikai.ingest.*` spans produced by ingest workers.
- Dashboards:
  - `dashboards/grafana/taikai_viewer.json` — viewer telemetry (latency, live
    edge, rebuffer rate, CEK fetch status) populated by the ingest collectors
    and the `taikai_viewer` harness.
  - `dashboards/grafana/taikai_cache.json` — cache fill level, CDN hit-rate.
  - Alert packs under `dashboards/alerts/taikai_viewer_rules.yml`.

## 4. CLI & Automation Flows

| Tool | Purpose | Notes |
|------|---------|-------|
| `iroha app taikai bundle` / `sorafs_cli taikai bundle` | Offline bundling of CMAF fragments into CAR + TSE. | Available today; referenced by `docs/source/taikai_segment_envelope.md`. |
| `iroha app taikai ingest edge --payload fixtures/taikai/segments/ingest_edge_sample.m4s` | Prototype SRT/RTMP receiver that emits CMAF fragments, drift samples, and an NDJSON log so the watcher path can be rehearsed deterministically. | Writes fragments to `artifacts/taikai/ingest_edge_run_<stamp>/fragments/` plus `logs/ingest_edge.ndjson` and `logs/drift_monitor.json`; pass `--segments`, `--segment-interval-ms`, and `--drift-*` to shape the run. |
| `iroha app taikai ingest --watch <dir>` | Watches a fragment directory, derives manifest/storage-ticket digests, emits `/v1/da/ingest` metadata, and invokes the bundler per segment. | Emits CAR archives, envelopes, indexes, ingest metadata, and Norito DA requests under `artifacts/taikai/ingest_run_<timestamp>/`. The new `--publish-da` flag streams those requests directly to Torii (`/v1/da/ingest`) using the CLI config, while `--da-*` knobs expose lane/codec/retention overrides. Pass `--summary-out ingest_summary.ndjson` to capture a replayable NDJSON log (payload path, CAR/envelope outputs, manifest/storage-ticket digests, DA request receipts) so regulators and operators can regenerate bundles from the recorded evidence. |
| `scripts/taikai_ingest_smoke.sh` | Smoke harness that replays `fixtures/taikai/segments/*.json` via `taikai_car`, validates CAR/index/ingest outputs, and fails fast on digest or metadata drift. | Emits artefacts under `artifacts/taikai/ingest_smoke/<label>` with CLI stdout, CAR, Norito, index, and ingest-metadata snapshots for evidence bundles. |
| `integration_tests/tests/taikai_da.rs` | End-to-end ingest + DA fetch/regeneration tests. | Ensures manifest commitments match envelopes and caches. |

#### NDJSON summary log (`--summary-out`)

Appending `--summary-out <path>` to the ingest watcher emits an NDJSON artefact alongside
the usual CAR/envelope outputs. Each line captures the payload source, CAR/envelope/index/metadata
paths, manifest/storage-ticket digests, CAR CID/digest/size, drift and latency readings,
sequence/PT metadata, and the DA request/receipt locations. This log gives release/compliance
teams a machine-readable manifest for SN13-B: feed it back into automation (or archive it
for governance) and the Taikai bundles can be regenerated deterministically even after the raw
CMAF directory has aged out of hot storage.

When only the CAR archive and summary remain, `taikai_car` can rehydrate every artefact:
`taikai_car --car-in <segment.car> --car-out <out.car> --envelope-out <out.to> --indexes-out <out.indexes.json> --ingest-metadata-out <out.ingest.json> --summary-in <summary.json|ndjson> [--summary-entry N]`
rebuilds the envelope/index/ingest metadata without reading the originating CMAF fragment.

Run `scripts/taikai_ingest_smoke.sh` whenever the bundler, manifest schema, or
fixtures change. The helper builds `taikai_car` when needed, iterates over the
fixture set, and drops labelled evidence bundles under
`artifacts/taikai/ingest_smoke/`. Operators can attach the stdout digest log,
CAR archive, Norito envelope, and ingest/index JSON to SN13-A governance packs
without hand-written repro steps. CI glue simply invokes the script and uploads
the resulting directory as an artefact.

## 5. Fixtures & Deterministic Tests

- **Fixtures directory:** `fixtures/taikai/segments/`
  - JSON fixtures define payload bytes, CLI args, and expected digests/metadata.
    `smoke_video.json` mirrors the CLI example above; extend the directory with
    ladder-specific payloads, audio tracks, and metadata variations to expand
    coverage for the smoke harness.
- **Integration tests:** `integration_tests/tests/taikai_da.rs`
  - Submit fixtures through Torii ingest (using mock SRT/RTMP input).
  - Verify DA manifests, CAR pointers, and viewer cache hydration.
  - Reconstruct playback using Norito envelopes only.
- **Property tests:** `crates/sorafs_car/tests/taikai_bundle.rs`
  - Randomized payload sizes ensure CAR builder enforces length/digest rules.

## 6. Observability & Dashboards

| Asset | Purpose | Status |
|-------|---------|--------|
| `dashboards/grafana/taikai_viewer.json` | Viewer metrics (latency, live edge, rebuffer, CEK fetch, PQ circuit health). | ✅ Exported via `scripts/grafana/export_taikai_viewer.py`; populated by `taikai_ingest_*` and `taikai_viewer_*` metrics (emitted by the ingest collectors and the `taikai_viewer` CLI) so operators can replay telemetry locally or in CI. |
| `dashboards/grafana/taikai_cache.json` | Cache occupancy per PoP, prefetch latency, CAA propagation. | ✅ Exported dashboard referencing the `taikai_cache_*` metrics; edit in Grafana (same export flow as the viewer board) or modify the JSON directly when telemetry label sets change. |
| `dashboards/alerts/taikai_viewer_rules.yml` | Alert contract (LiveEdgeDrift, IngestFailure, CEKRotationLag). | ✅ Prometheus rules with promtool tests (`dashboards/alerts/tests/taikai_viewer_rules.test.yml`) seeded by the ingest + viewer metric fixtures. |

> **Telemetry label:** set `torii.da_ingest.telemetry_cluster_label` in `iroha_config`
> so `cluster` filters in the dashboard map to the deployment (e.g., `staging-apac`).

## 7. Acceptance Checklist (SN13-A)

| Requirement | Evidence |
|-------------|----------|
| SRT/RTMP receivers authenticate publishers and emit drift samples. | Config + smoke logs stored under `artifacts/taikai/ingest_edge_run_<stamp>/logs/ingest_edge.ndjson` (paired with `drift_monitor.json`). |
| Ladder builder produces deterministic CMAF fragments for required renditions. | Fixture comparison in `scripts/taikai_ingest_smoke.sh`. |
| Bundler generates CAR/TSE pairs with matching digests and ingest metadata. | CLI output + checksums captured in release bundles. |
| Drift monitors feed telemetry + alerts. | Metrics exported to `dashboards/grafana/taikai_viewer.json`; alert pack lives in `dashboards/alerts/taikai_viewer_rules.yml` with promtool coverage driven by ingest/viewer metric fixtures. |
| Integration tests replay ingest → DA → viewer caches. | `cargo test -p integration_tests taikai_da::` logs attached to roadmap evidence. |

## 8. Completion Notes

- `iroha app taikai ingest edge` now seeds CMAF fragments and drift samples from a
  single payload (see `fixtures/taikai/segments/ingest_edge_sample.m4s`), writing
  evidence under `artifacts/taikai/ingest_edge_run_<stamp>/` so the watcher path
  can be rehearsed without live SRT/RTMP traffic.
- Ladder presets and the smoke harness are checked in (`fixtures/taikai/ladder_presets.json`,
  `scripts/taikai_ingest_smoke.sh`, `fixtures/taikai/segments/smoke_video.json`),
  letting operators regenerate CAR/index/ingest outputs deterministically.
- The ingest watcher continues to emit `/v1/da/ingest` payloads and optional
  receipts (`--publish-da`); dashboards and alert packs are published alongside
  the Taikai viewer/cache telemetry bundles.
- Roadmap/status entries were updated to mark SN13-A as complete with the new
  ingest edge evidence paths captured for governance packets.
