<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Taikai Publisher Sample (SN13-F)

This example bootstraps roadmap task **SN13-F — Publisher SDK/CLI** by
demonstrating how to bundle a Taikai segment with the standalone
`taikai_car` binary (or the equivalent `sorafs_cli taikai bundle` command).
It ships a deterministic payload, Norito metadata, and a Python helper that
stitches the CLI arguments together so media teams can
reproduce the workflow without wiring up the full ingest pipeline.

The sample lives outside the Cargo workspace to avoid pulling extra crates into
`Cargo.toml`. Tests in `crates/sorafs_car/tests/taikai_car_cli.rs` exercise the
same assets to keep this directory verified during `cargo test`.

## Prerequisites

- Rust toolchain capable of building `sorafs_car`.
- A Taikai bundler CLI: either the standalone `taikai_car` binary in `PATH`
  (or `TAIKAI_CAR_BIN=/path/to/taikai_car`) or the `sorafs_cli taikai bundle`
  command from the SoraFS toolkit.
- Python 3.8+ for the helper script.

Build the CLI once:

```bash
cargo build -p sorafs_car --bin taikai_car
```

## Bundle the sample segment

```bash
cd sdk/examples/taikai_publisher
python3 bundle_sample.py --print-command
```

Output artefacts land under `artifacts/sample_run/`:

| File | Description |
|------|-------------|
| `sample_segment_0001.car` | Deterministic CARv2 archive containing the mock payload. |
| `sample_segment_0001.norito` | Norito-encoded `TaikaiSegmentEnvelopeV1`. |
| `sample_segment_0001.indexes.json` | JSON time/CID index bundle consumed by anchors. |
| `sample_segment_0001.ingest.json` | Metadata map ready for `/v1/da/ingest`. |
| `sample_segment_0001.summary.json` | Optional bundle summary (`taikai_car --summary-out …`) containing the CAR digest, chunk info, ingest metadata, and output paths for automation. |

The script prints a summary derived from the ingest metadata so operators can
double-check event/stream/rendition IDs before uploading. Pass
`--summary-out /tmp/sample_segment.summary.json` when invoking `taikai_car`
directly (or `sorafs_cli taikai bundle`) to capture the same information (plus
CAR/Chunk digests and output paths) in machine-readable form—automation can
ingest the file without scraping stdout. The helper’s `--summary-json` flag
simply aggregates the per-bundle summaries into one manifest for multi-config
runs.

## Configuration files

- [`sample_config.json`](./sample_config.json) — declarative parameters passed to
  `taikai_car` (event/stream IDs, codec, manifest hash, etc.). Paths are
  relative to this directory so the config remains portable.
- [`fixtures/sample_segment_0001.m4s`](./fixtures/sample_segment_0001.m4s) —
  deterministic placeholder payload.
- [`sample_config_720p.json`](./sample_config_720p.json) — alternate ladder rung
  used to prove multi-config automation in tests.
- [`sample_config_audio.json`](./sample_config_audio.json) — stereo audio config
  that drives the same helper/CLI path while exercising the audio layout
  surfaces (`--track-kind audio --audio-layout stereo`).
- [`fixtures/sample_segment_audio_0001.m4s`](./fixtures/sample_segment_audio_0001.m4s) —
  deterministic payload paired with the audio config.
- [`fixtures/sample_metadata.json`](./fixtures/sample_metadata.json) — optional
  Norito metadata file referenced via `--metadata-json`.

Adjust the JSON config to point at real CMAF fragments or alternate ladder
settings. The helper automatically derives output filenames and keeps the CLI
arguments in sync with the config.

## Automation hook-up

CI/unattended environments can invoke the helper directly:

```bash
python3 bundle_sample.py \
  --config sample_config.json \
  --out-dir /tmp/taikai-run \
  --taikai-car target/debug/taikai_car
```

Because the integration test imports the same assets, any future change to this
directory must be reflected in the test to keep the roadmap deliverable
deterministic.

## Bundle multiple ladder rungs

To exercise SN13-F’s multi-rendition automation path, pass multiple `--config`
arguments. The script now iterates over every config, prints a per-bundle
summary, and can emit an aggregate manifest via `--summary-json`:

```bash
python3 bundle_sample.py \
  --config sample_config.json \
  --config sample_config_720p.json \
  --config sample_config_audio.json \
  --summary-json /tmp/taikai-run/summary.json \
  --out-dir /tmp/taikai-run \
  --taikai-car target/debug/taikai_car
```

The repository ships `sample_config.json` (1080p),
`sample_config_720p.json` (720p), and `sample_config_audio.json`
(`audio-stereo`) so publishers can rehearse ladder + audio workflows without
duplicating payload definitions. Each config uses a distinct CMAF fixture so
output filenames remain stable across runs.

## Rehydrate from a stored CAR

If the CMAF fragment has been deleted but you still have the CAR archive and
summary output, regenerate the envelope/index/ingest artefacts without
re-bundling the payload:

```bash
taikai_car \
  --car-in artifacts/sample_run/sample_segment_0001.car \
  --car-out /tmp/taikai-regenerated/sample_segment_0001.car \
  --envelope-out /tmp/taikai-regenerated/sample_segment_0001.norito \
  --indexes-out /tmp/taikai-regenerated/sample_segment_0001.indexes.json \
  --ingest-metadata-out /tmp/taikai-regenerated/sample_segment_0001.ingest.json \
  --summary-in artifacts/sample_run/sample_segment_0001.summary.json
```

When the summary contains multiple entries (arrays or NDJSON), pass
`--summary-entry <index>` to pick the correct bundle record.

## Validate playback + CEK telemetry (taikai_viewer)

The new `taikai_viewer` helper (shipped from the `sorafs_orchestrator` crate)
consumes the bundled artefacts and emits Prometheus metrics used by
`dashboards/grafana/taikai_viewer.json`:

```bash
cargo build -p sorafs_orchestrator --bin taikai_viewer
target/debug/taikai_viewer \
  --segment envelope=artifacts/sample_run/sample_segment_0001.norito,car=artifacts/sample_run/sample_segment_0001.car \
  --segment envelope=artifacts/sample_run/sample_segment_0001_720p.norito,car=artifacts/sample_run/sample_segment_0001_720p.car \
  --cluster demo \
  --lane lane-main \
  --rebuffer-events 1 \
  --pq-health 95 \
  --metrics-out /tmp/taikai_viewer.prom \
  --summary-out /tmp/taikai_viewer_summary.json
```

Add `--cek-receipt <path>` (and optionally `--cek-fetch-ms <override>`) to
record CEK fetch/rotation telemetry when receipts are available. The metrics
file can be scraped directly, and the JSON summary captures the validated
segments plus CEK/pq/rebuffer evidence for roadmap SN13-G.
