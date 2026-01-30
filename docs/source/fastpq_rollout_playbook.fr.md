---
lang: fr
direction: ltr
source: docs/source/fastpq_rollout_playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3a0c22a213e04a6a8fef94ded6ec0017531737ffd4b9418ec94286bb6759ff8a
source_last_modified: "2026-01-08T09:26:20.579700+00:00"
translation_last_reviewed: 2026-01-30
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# FASTPQ Rollout Playbook (Stage 7-3)

This playbook implements the Stage 7-3 roadmap requirement: every fleet upgrade
that enables FASTPQ GPU execution must attach a reproducible benchmark manifest,
paired Grafana evidence, and a documented rollback drill. It complements
`docs/source/fastpq_plan.md` (targets/architecture) and
`docs/source/fastpq_migration_guide.md` (node-level upgrade steps) by focusing
on the operator-facing rollout checklist.

## Scope & Roles

- **Release Engineering / SRE:** own benchmark captures, manifest signing, and
  dashboard exports prior to rollout approval.
- **Ops Guild:** runs staged rollouts, records rollback rehearsals, and stores
  the artefact bundle under `artifacts/fastpq_rollouts/<timestamp>/`.
- **Governance / Compliance:** verifies that evidence accompanies every change
  request before the FASTPQ default is toggled for a fleet.

## Evidence Bundle Requirements

Every rollout submission must contain the following artefacts. Attach all files
to the release/upgrade ticket and keep the bundle in
`artifacts/fastpq_rollouts/<YYYYMMDD>/<fleet>/<lane>/`.

| Artefact | Purpose | How to produce |
|----------|---------|----------------|
| `fastpq_bench_manifest.json` | Proves that the canonical 20 000-row workload stays under the `<1 s` LDE ceiling and records hashes for every wrapped benchmark.| Capture Metal/CUDA runs, wrap them, then run:<br>`cargo xtask fastpq-bench-manifest \`<br>`  --bench metal=artifacts/fastpq_benchmarks/<metal>.json \`<br>`  --bench cuda=artifacts/fastpq_benchmarks/<cuda>.json \`<br>`  --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json \`<br>`  --signing-key secrets/fastpq_bench.ed25519 \`<br>`  --out artifacts/fastpq_rollouts/<stamp>/fastpq_bench_manifest.json` |
| Wrapped benchmarks (`fastpq_metal_bench_*.json`, `fastpq_cuda_bench_*.json`) | Capture host metadata, row-usage evidence, zero-fill hotspots, Poseidon microbench summaries, and kernel statistics used by dashboards/alerts.| Run `fastpq_metal_bench` / `fastpq_cuda_bench`, then wrap the raw JSON:<br>`python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 \`<br>`  --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json \`<br>`  --poseidon-metrics artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/metrics_poseidon.prom \`<br>`  fastpq_metal_bench.json artifacts/fastpq_benchmarks/<metal>.json --sign-output`<br>Repeat for CUDA captures (point `--row-usage` and `--poseidon-metrics` at the relevant witness/scrape files). The helper embeds the filtered `fastpq_poseidon_pipeline_total`/`fastpq_execution_mode_total` samples so WP2-E.6 evidence is identical across Metal and CUDA. Use `scripts/fastpq/export_poseidon_microbench.py --bundle <bundle>` when you need a standalone Poseidon microbench summary (wrapped or raw inputs supported). |
|  |  | **Stage7 label requirement:** `wrap_benchmark.py` now fails unless the resulting `metadata.labels` section contains both `device_class` and `gpu_kind`. When automatic detection cannot infer them (for example, when wrapping on a detached CI node), pass explicit overrides such as `--label device_class=xeon-rtx-sm80 --label gpu_kind=discrete`. |
|  |  | **Acceleration telemetry:** the wrapper also captures `cargo xtask acceleration-state --format json` by default, writing `<bundle>.accel.json` and `<bundle>.accel.prom` next to the wrapped benchmark (override with `--accel-*` flags or `--skip-acceleration-state`). The capture matrix uses these files to build `acceleration_matrix.{json,md}` for fleet dashboards. |
| Grafana export | Proves adoption telemetry and alert annotations for the rollout window.| Export the `fastpq-acceleration` dashboard:<br>`curl -s -H "Authorization: Bearer $GRAFANA_TOKEN" \`<br>`  "$GRAFANA_URL/api/dashboards/uid/fastpq-acceleration" \`<br>`  | jq '.dashboard' \`<br>`  > artifacts/fastpq_rollouts/<stamp>/grafana_fastpq_acceleration.json`<br>Annotate the board with rollout start/stop times before exporting. The release pipeline can do this automatically via `scripts/run_release_pipeline.py --export-fastpq-grafana --grafana-url <URL>` (token supplied via `GRAFANA_TOKEN`). |
| Alert snapshot | Captures the alert rules that guarded the rollout.| Copy `dashboards/alerts/fastpq_acceleration_rules.yml` (and the `tests/` fixture) into the bundle so reviewers can re-run `promtool test rules …`. |
| Rollback drill log | Demonstrates that operators rehearsed the forced CPU fallback and telemetry acknowledgements.| Use the procedure in [Rollback Drills](#rollback-drills) and store console logs (`rollback_drill.log`) plus the resulting Prometheus scrape (`metrics_rollback.prom`). |
| `row_usage/fastpq_row_usage_<date>.json` | Records the ExecWitness FASTPQ row allocation that TF-5 tracks in CI and dashboards.| Download a fresh witness from Torii, decode it via `iroha_cli audit witness --decode exec.witness` (optionally add `--fastpq-parameter fastpq-lane-balanced` to assert the expected parameter set; FASTPQ batches emit by default), and copy the `row_usage` JSON into `artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/row_usage/`. Keep filenames timestamped so reviewers can correlate them with the rollout ticket, and run `python3 scripts/fastpq/validate_row_usage_snapshot.py row_usage/*.json` (or `make check-fastpq-rollout`) so the Stage 7-3 gate verifies that every batch advertises the selector counts and `transfer_ratio = transfer_rows / total_rows` invariant before attaching the evidence. |

> **Tip:** `artifacts/fastpq_rollouts/README.md` documents the preferred naming
> scheme (`<stamp>/<fleet>/<lane>`) and the required evidence files. The
> `<stamp>` folder must encode `YYYYMMDDThhmmZ` so artefacts stay sortable
> without consulting tickets.

## Evidence Generation Checklist

1. **Capture GPU benchmarks.**
   - Run the canonical workload (20 000 logical rows, 32 768 padded rows) via
     `cargo run -p fastpq_prover --bin fastpq_metal_bench -- --rows 20000 --pretty`.
   - Wrap the result with `scripts/fastpq/wrap_benchmark.py` using `--row-usage <decoded witness>` so the bundle carries the gadget evidence alongside the GPU telemetry. Pass `--require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 --sign-output` so the wrapper fails fast if either accelerator exceeds the target or if the Poseidon queue/profile telemetry is missing, and to generate the detached signature.
   - Repeat on the CUDA host so the manifest contains both GPU families.
   - Do **not** strip the `benchmarks.metal_dispatch_queue` or
     `benchmarks.zero_fill_hotspots` blocks from the wrapped JSON. The CI gate
     (`ci/check_fastpq_rollout.sh`) now reads those fields and fails when queue
     headroom drops below one slot or when any LDE hotspot reports `mean_ms >
     0.40 ms`, enforcing the Stage 7 telemetry guard automatically.
2. **Generate the manifest.** Use `cargo xtask fastpq-bench-manifest …` as
   shown in the table. Store `fastpq_bench_manifest.json` in the rollout bundle.
3. **Export Grafana.**
   - Annotate the `FASTPQ Acceleration Overview` board with the rollout window,
     linking to the relevant Grafana panel IDs.
   - Export the dashboard JSON via the Grafana API (command above) and include
     the `annotations` section so reviewers can match adoption curves to the
     staged rollout.
4. **Snapshot alerts.** Copy the exact alert rules (`dashboards/alerts/…`) used
   by the rollout into the bundle. If Prometheus rules were overridden, include
   the override diff.
5. **Prometheus/OTEL scrape.** Capture `fastpq_execution_mode_total{device_class="<matrix>"}` from each
   host (before and after the stage) plus the OTEL counter
   `fastpq.execution_mode_resolutions_total` and the paired
   `telemetry::fastpq.execution_mode` log entries. These artefacts prove that
   GPU adoption is stable and that forced CPU fallbacks still emit telemetry.
6. **Archive row-usage telemetry.** After decoding the ExecWitness run for the
   rollout, drop the resulting JSON under `row_usage/` in the bundle. The CI
   helper (`ci/check_fastpq_row_usage.sh`) compares these snapshots against the
   canonical baselines, and `ci/check_fastpq_rollout.sh` now requires every
   bundle to ship at least one `row_usage` file to keep TF-5 evidence attached
   to the release ticket.

## Staged Rollout Flow

Use three deterministic phases for every fleet. Advance only after the exit
criteria in each phase are satisfied and documented in the evidence bundle.

| Phase | Scope | Exit Criteria | Attachments |
|-------|-------|---------------|-------------|
| Pilot (P1) | 1 control-plane + 1 data-plane node per region | `fastpq_execution_mode_total{device_class="<matrix>", backend="metal"}` ≥90% for 48 h, zero Alertmanager incidents, and a passing rollback drill. | Bundle from both hosts (bench JSONs, Grafana export with pilot annotation, rollback logs). |
| Ramp (P2) | ≥50% of validators plus at least one archival lane per cluster | GPU execution sustained for 5 days, no more than 1 downgrade spike >10 min, and Prometheus counters prove fallbacks alert within 60 s. | Updated Grafana export showing the ramp annotation, Prometheus scrape diffs, Alertmanager screenshot/log. |
| Default (P3) | Remaining nodes; FASTPQ marked default in `iroha_config` | Signed bench manifest + Grafana export referencing the final adoption curve, and documented rollback drill demonstrating the config toggle. | Final manifest, Grafana JSON, rollback log, ticket reference to config change review. |

Document each promotion step in the rollout ticket and link directly to the
`grafana_fastpq_acceleration.json` annotations so reviewers can correlate the
timeline with the evidence.

## Rollback Drills

Every rollout stage must include a rollback rehearsal:

1. Pick one node per cluster and record the current metrics:
   ```bash
   curl -s http://<host>:8180/metrics | rg 'fastpq_execution_mode_total{device_class'
   ```
2. Force CPU mode for 10 minutes using either the config knob
   (`zk.fastpq.execution_mode = "cpu"`) or the environment override:
   ```bash
   FASTPQ_GPU=cpu irohad --config <path> --genesis-manifest-json <path>
   ```
3. Confirm the downgrade log
   (`telemetry::fastpq.execution_mode resolved="cpu" requested="gpu"`) and scrape
   the Prometheus endpoint again to show the counter increments.
4. Restore GPU mode, verify that `telemetry::fastpq.execution_mode` now reports
   `resolved="metal"` (or `resolved="cuda"/"opencl"` for non-Metal lanes),
   confirm the Prometheus scrape contains both the CPU and GPU samples in
   `fastpq_execution_mode_total{backend=…}`, and log the elapsed time to
   detection/cleanup.
5. Store shell transcripts, metrics, and operator acknowledgements as
   `rollback_drill.log` and `metrics_rollback.prom` in the rollout bundle. These
   files must illustrate the full downgrade + restore cycle because
   `ci/check_fastpq_rollout.sh` now fails whenever the log lacks the GPU
   recovery line or the metrics snapshot omits either the CPU or GPU counters.

These logs prove that every cluster can degrade gracefully and that SRE teams
know how to fall back deterministically if GPU drivers or kernels regress.

## Mixed-mode fallback evidence (WP2-E.6)

Whenever a host needs GPU FFT/LDE but CPU Poseidon hashing (per the Stage 7 <900 ms
requirement), bundle the following artefacts alongside the standard rollback logs:

1. **Config diff.** Check in (or attach) the host-local override that sets
   `zk.fastpq.poseidon_mode = "cpu"` (`FASTPQ_POSEIDON_MODE=cpu`) while leaving
   `zk.fastpq.execution_mode` untouched. Name the patch
   `artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/poseidon_fallback.patch`.
2. **Poseidon counter scrape.**
   ```bash
   curl -s http://<host>:8180/metrics \
     | rg 'fastpq_poseidon_pipeline_total{.*device_class="<label>"' \
     > artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/metrics_poseidon.prom
   ```
   The capture must show `path="cpu_forced"` incrementing in lock-step with the
   GPU FFT/LDE counter for that device-class. Take a second scrape after reverting
   back to GPU mode so reviewers can see the `path="gpu"` row resume.

   Pass the resulting file to `wrap_benchmark.py --poseidon-metrics …` so the wrapped benchmark records the same counters inside its `poseidon_metrics` section; this keeps Metal and CUDA rollouts on the identical workflow and makes the fallback evidence auditable without opening separate scrape files.
3. **Log excerpt.** Copy the `telemetry::fastpq.poseidon` entries that prove the
   resolver flipped to CPU (`cpu_forced`) into
   `poseidon_fallback.log`, keeping timestamps so Alertmanager timelines can be
   correlated with the config change.

CI enforces the queue/zero-fill checks today; once the mixed-mode gate lands,
`ci/check_fastpq_rollout.sh` will also insist that any bundle containing
`poseidon_fallback.patch` ships the matching `metrics_poseidon.prom` snapshot.
Following this workflow keeps the WP2-E.6 fallback policy auditable and tied to
the same evidence collectors used during the default-on rollout.

## Reporting & Automation

- Attach the entire `artifacts/fastpq_rollouts/<stamp>/` directory to the
  release ticket and reference it from `status.md` once the rollout closes.
- Run `dashboards/alerts/tests/fastpq_acceleration_rules.test.yml` (via
  `promtool`) inside CI to ensure alert bundles bundled with the rollout still
  compile.
- Validate the bundle with `ci/check_fastpq_rollout.sh` (or
  `make check-fastpq-rollout`) and pass `FASTPQ_ROLLOUT_BUNDLE=<path>` when you
  want to target a single rollout. CI invokes the same script via
  `.github/workflows/fastpq-rollout.yml`, so missing artefacts fail fast before a
  release ticket can close. The release pipeline can archive validated bundles
  alongside the signed manifests by passing
  `--fastpq-rollout-bundle artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>` to
  `scripts/run_release_pipeline.py`; the helper reruns
  `ci/check_fastpq_rollout.sh` (unless `--skip-fastpq-rollout-check` is set) and
  copies the directory tree into `artifacts/releases/<version>/fastpq_rollouts/…`.
  As part of this gate the script enforces the Stage 7 queue-depth and zero-fill
  budgets by reading `benchmarks.metal_dispatch_queue` and
  `benchmarks.zero_fill_hotspots` out of each `metal` bench JSON.

By following this playbook we can demonstrate deterministic adoption, provide a
single evidence bundle per rollout, and keep rollback drills audited alongside
the signed benchmark manifests.
