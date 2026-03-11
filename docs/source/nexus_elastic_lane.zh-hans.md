---
lang: zh-hans
direction: ltr
source: docs/source/nexus_elastic_lane.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7260599c2d279f79158a7ffa13fe5cf4af2864ce3f842ecb2a812ee50e045f64
source_last_modified: "2026-01-22T16:26:46.577291+00:00"
translation_last_reviewed: 2026-02-07
title: Elastic Lane Provisioning (NX-7)
sidebar_label: Elastic Lane Provisioning
description: Bootstrap workflow for creating Nexus lane manifests, catalog entries, and rollout evidence.
---

# Elastic Lane Provisioning Toolkit (NX-7)

> **Roadmap item:** NX-7 — Elastic lane provisioning tooling  
> **Status:** Tooling complete — generates manifests, catalog snippets, Norito payloads, smoke tests,
> and the load-test bundle helper now stitches slot latency gating + evidence manifests so validator
> load runs can be published without bespoke scripting.

This guide walks operators through the new `scripts/nexus_lane_bootstrap.sh` helper that automates
lane manifest generation, lane/dataspace catalog snippets, and rollout evidence. The goal is to make
it easy to spin up new Nexus lanes (public or private) without hand-editing multiple files or
re-deriving the catalog geometry by hand.

## 1. Prerequisites

1. Governance approval for the lane alias, dataspace, validator set, fault tolerance (`f`), and settlement policy.
2. A finalized validator list (account IDs) and protected namespace list.
3. Access to the node configuration repository so you can append the generated snippets.
4. Paths for the lane manifest registry (see `nexus.registry.manifest_directory` and
   `cache_directory`).
5. Telemetry contacts/PagerDuty handles for the lane so alerts can be wired as soon as the lane
   comes online.

## 2. Generate lane artefacts

Run the helper from the repository root:

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator i105... \
  --validator i105... \
  --validator i105... \
  --protected-namespace payments \
  --description "High-throughput interbank payments lane" \
  --dataspace-description "Payments dataspace" \
  --route-instruction finance::pacs008 \
  --encode-space-directory \
  --space-directory-out artifacts/nexus/payments_lane/payments.manifest.to \
  --telemetry-contact payments-ops@sora.org \
  --output-dir artifacts/nexus/payments_lane
```

Key flags:

- `--lane-id` must match the new entry’s index in `nexus.lane_catalog`.
- `--dataspace-alias` and `--dataspace-id/hash` control the dataspace catalog entry (defaults to the
  lane id when omitted).
- `--validator` can be repeated or sourced from `--validators-file`.
- `--route-instruction` / `--route-account` emit ready-to-paste routing rules.
- `--metadata key=value` (or `--telemetry-contact/channel/runbook`) capture runbook contacts so
  dashboards immediately list the right owners.
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` add the runtime-upgrade hook to the manifest
  when the lane requires extended operator controls.
- `--encode-space-directory` invokes `cargo xtask space-directory encode` automatically. Pair it with
  `--space-directory-out` when you want the encoded `.to` file somewhere other than the default.

The script produces three artefacts inside the `--output-dir` (defaults to the current directory),
plus an optional fourth when encoding is enabled:

1. `<slug>.manifest.json` — lane manifest containing the validator quorum, protected namespaces, and
   optional runtime-upgrade hook metadata.
2. `<slug>.catalog.toml` — a TOML snippet with `[[nexus.lane_catalog]]`, `[[nexus.dataspace_catalog]]`,
   and any requested routing rules. Ensure `fault_tolerance` is set on the dataspace entry to size
   the lane-relay committee (`3f+1`).
3. `<slug>.summary.json` — audit summary describing the geometry (slug, segments, metadata) plus the
   required rollout steps and the exact `cargo xtask space-directory encode` command (under
   `space_directory_encode.command`). Attach this JSON to the onboarding ticket for evidence.
4. `<slug>.manifest.to` — emitted when `--encode-space-directory` is set; ready for Torii’s
   `iroha app space-directory manifest publish` flow.

Use `--dry-run` to preview the JSON/ snippets without writing files, and `--force` to overwrite
existing artefacts.

## 3. Apply the changes

1. Copy the manifest JSON into the configured `nexus.registry.manifest_directory` (and into the cache
   directory if the registry mirrors remote bundles). Commit the file if manifests are versioned in
   your configuration repo.
2. Append the catalog snippet to `config/config.toml` (or the appropriate `config.d/*.toml`). Ensure
   `nexus.lane_count` is at least `lane_id + 1`, and update any `nexus.routing_policy.rules` that
   should point at the new lane.
3. Encode (if you skipped `--encode-space-directory`) and publish the manifest to the Space Directory
   using the command captured in the summary (`space_directory_encode.command`). This produces the
   `.manifest.to` payload Torii expects and records the evidence for auditors; submit via
   `iroha app space-directory manifest publish`.
4. Run `irohad --sora --config path/to/config.toml --trace-config` and archive the trace output in
   the rollout ticket. This proves the new geometry matches the generated slug/kura segments.
5. Restart the validators assigned to the lane once the manifest/catalog changes are deployed. Keep
   the summary JSON in the ticket for future audits.

## 4. Build a registry distribution bundle

Once the manifest, catalog snippet, and summary are ready, package them for distribution to your
validators. The new bundler script copies manifests into the layout expected by
`nexus.registry.manifest_directory` / `cache_directory`, emits a governance catalog overlay so
modules can be swapped without editing the main config, and optionally archives the bundle:

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

Results:

1. `manifests/<slug>.manifest.json` — copy these into `nexus.registry.manifest_directory`.
2. `cache/governance_catalog.json` — drop into `nexus.registry.cache_directory` to override or swap
   governance modules (`--module …` entries override the cached catalog). This is the pluggable
   module path for NX-2: replace a module definition, re-run the bundler, and distribute the cache
   overlay without touching `config.toml`.
3. `summary.json` — includes SHA-256 / Blake2b digests for every manifest plus the overlay metadata.
4. Optional `registry_bundle.tar.*` archive — ready for Secure Copy / artifact storage.

If your deployment mirrors bundles to air‑gapped hosts, sync the entire output directory (or the
generated tarball). Online nodes can mount the manifest directory directly while offline nodes
consume the tarball, extract it, and copy the manifests + cache overlay into their configured paths.

## 5. Validator smoke tests

After Torii restarts, run the new smoke helper to verify the lane reports `manifest_ready=true`,
metrics expose the expected lane count, and the sealed gauge is clear. Lanes that require a manifest
must now expose a non-empty `manifest_path`—the helper fails fast when that evidence is missing so
NX-7 change controls include the signed bundle references automatically:

```bash
scripts/nexus_lane_smoke.py \
  --status-url https://torii.example.com/v1/sumeragi/status \
  --metrics-url https://torii.example.com/metrics \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --min-teu-capacity 64 \
  --max-teu-slot-commit-ratio 0.85 \
  --max-teu-deferrals 0 \
  --max-must-serve-truncations 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

Add `--insecure` when testing self-signed environments. The script exits non-zero if the lane is
missing, sealed, or metrics/telemetry drift from the expected values. Use the new `--min-block-height`,
`--max-finality-lag`, `--max-settlement-backlog`, and `--max-headroom-events` knobs to keep per-lane
block height/finality/backlog/headroom telemetry within your operational envelopes. Pair them with
`--max-slot-p95/--max-slot-p99` (plus `--min-slot-samples`) to enforce the NX-18 slot-duration SLO
directly inside the smoke helper, and pass
`--allow-missing-lane-metrics` only when staging clusters have not yet exposed those gauges
(production evidence should keep the defaults enforced).

The same helper now enforces scheduler load-test telemetry. Use `--min-teu-capacity` to prove each
lane reports a non-zero `nexus_scheduler_lane_teu_capacity`, gate the slot utilisation with
`--max-teu-slot-commit-ratio` (compares `nexus_scheduler_lane_teu_slot_committed` against capacity),
and keep the deferral/truncation counters at zero via `--max-teu-deferrals` and
`--max-must-serve-truncations`. These knobs turn the NX-7 "deeper validator load tests" requirement
into a repeatable CLI check: the smoke helper fails when a lane defers PQ/TEU work or when the
committed TEU per slot creeps beyond the configured headroom, and the CLI prints the per-lane
capacity/ratio/deferral summary so evidence packets capture the same numbers that CI validated.

After the smoke run succeeds, archive the Prometheus snapshot and slot summary with
`scripts/telemetry/bundle_slot_artifacts.py --metrics <metrics.prom> --summary artifacts/nx18/slot_summary.json --out-dir artifacts/nx18`.
The helper copies both artefacts into `artifacts/nx18/` and emits `slot_bundle_manifest.json`
with SHA-256 digests so the NX-18 evidence bundle contains exactly what the gate evaluated.

For air-gapped validations (or CI) you can replay a captured Torii response instead of hitting a live
endpoint:

```bash
scripts/nexus_lane_smoke.py \
  --status-file fixtures/nexus/lanes/status_ready.json \
  --metrics-file fixtures/nexus/lanes/metrics_ready.prom \
  --lane-alias core \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --min-teu-capacity 64 \
  --max-teu-slot-commit-ratio 0.85 \
  --max-teu-deferrals 0 \
  --max-must-serve-truncations 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

The recorded fixtures under `fixtures/nexus/lanes/` mirror the artefacts produced by the bootstrap
helper so new manifests can be linted without bespoke scripting. CI exercises the same flow via
`ci/check_nexus_lane_smoke.sh` and also runs `ci/check_nexus_lane_registry_bundle.sh`
(alias: `make check-nexus-lanes`) to prove the NX-7 smoke helper stays aligned with the published
payload format and to ensure bundle digests/overlays remain reproducible.

When a lane is renamed, capture the `nexus.lane.topology` telemetry events (for example with
`journalctl -u irohad -o json | jq 'select(.msg=="nexus.lane.topology")'`) and feed them back into
the smoke helper. The new `--telemetry-file/--from-telemetry` flag accepts the newline-delimited log
and `--require-alias-migration old:new` asserts that a `alias_migrated` event recorded the rename:

```bash
scripts/nexus_lane_smoke.py \
  --status-file fixtures/nexus/lanes/status_ready.json \
  --metrics-file fixtures/nexus/lanes/metrics_ready.prom \
  --telemetry-file fixtures/nexus/lanes/telemetry_alias_migrated.ndjson \
  --lane-alias payments \
  --lane-alias core \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10 \
  --require-alias-migration core:payments
```

The `telemetry_alias_migrated.ndjson` fixture bundles the canonical rename sample so CI can verify
the telemetry parsing path without contacting a live node.

## 6. Validator load tests (NX-7 evidence)

Roadmap **NX-7** requires lane operators to capture a reproducible validator load run before a new
lane is marked production ready. The goal is to stress the lane long enough to exercise slot
duration, settlement backlog, DA quorum, oracle, scheduler headroom, and TEU metrics, then archive
the result in a way that auditors can replay without bespoke tooling. The new
`scripts/nexus_lane_load_test.py` helper stitches the smoke checks, slot-duration gating, and slot
bundle manifest into one artefact set so load runs can be published directly to governance tickets.

### 6.1 Workload preparation

1. Create a run directory and capture canonical fixtures for the lane under test:

   ```bash
   mkdir -p artifacts/nexus/load/payments-2026q2
   cargo xtask nexus-fixtures --output artifacts/nexus/load/payments-2026q2/fixtures
   ```

   The fixtures mirror `fixtures/nexus/lane_commitments/*.json` and give the load generator a
   deterministic workload seed (record the seed in `artifacts/.../README.md`).
2. Baseline the lane before the run:

   ```bash
   scripts/nexus_lane_smoke.py \
     --status-url https://torii.example.com/v1/sumeragi/status \
     --metrics-url https://torii.example.com/metrics \
     --lane-alias payments \
     --expected-lane-count 3 \
     --min-block-height 50000 \
     --max-finality-lag 4 \
     --max-settlement-backlog 0.5 \
     --min-settlement-buffer 0.25 \
     --max-slot-p95 1000 \
     --max-slot-p99 1100 \
     --min-slot-samples 50 \
     --insecure \
     > artifacts/nexus/load/payments-2026q2/smoke_before.log
   ```

   Keep the stdout/stderr in the run directory so the smoke thresholds are auditable.
3. Capture the telemetry log that will later feed `--telemetry-file` (alias migration evidence) and
   `validate_nexus_telemetry_pack.py`:

   ```bash
   journalctl -u irohad -o json \
     --since "2026-05-10T09:00:00Z" \
     --until "2026-05-10T11:00:00Z" \
     > artifacts/nexus/load/payments-2026q2/nexus.lane.topology.ndjson
   ```

4. Start the lane workload (k6 profile, replay harness, or the federation-wide ingestion tests) and
   keep the workload seed + slot range handy; the metadata is consumed by the telemetry manifest
   validator in §6.3.

5. Package the load-run evidence with the new helper. Supply the captured status/metrics/telemetry
   payloads, the lane aliases, and any alias-migration events that should appear in telemetry. The
   helper writes `smoke.log`, `slot_summary.json`, a slot bundle manifest, and `load_test_manifest.json`
   tying everything together for governance review:

   ```bash
   scripts/nexus_lane_load_test.py \
     --status-file artifacts/nexus/load/payments-2026q2/torii_status.json \
     --metrics-file artifacts/nexus/load/payments-2026q2/metrics.prom \
     --telemetry-file artifacts/nexus/load/payments-2026q2/nexus.lane.topology.ndjson \
     --lane-alias payments \
     --lane-alias core \
     --expected-lane-count 3 \
     --slot-range 81200-81600 \
     --workload-seed NX7-PAYMENTS-2026Q2 \
     --require-alias-migration core:payments \
     --out-dir artifacts/nexus/load/payments-2026q2
   ```

   The command enforces the same DA quorum, oracle, settlement buffer, TEU, and slot-duration gates
   used elsewhere in this guide and produces a ready-to-attach manifest without bespoke scripting.

### 6.2 Instrumented run

While the workload is saturating the lane:

1. Snapshot Torii status + metrics:

   ```bash
   curl -sS https://torii.example.com/v1/sumeragi/status \
     > artifacts/nexus/load/payments-2026q2/torii_status.json
   curl -sS https://torii.example.com/metrics \
     > artifacts/nexus/load/payments-2026q2/metrics.prom
   ```

2. Compute slot-duration quantiles and archive the summary:

   ```bash
   scripts/telemetry/check_slot_duration.py \
     artifacts/nexus/load/payments-2026q2/metrics.prom \
     --max-p95-ms 1000 \
     --max-p99-ms 1100 \
     --min-samples 200 \
     --json-out artifacts/nexus/load/payments-2026q2/slot_summary.json
   scripts/telemetry/bundle_slot_artifacts.py \
     --metrics artifacts/nexus/load/payments-2026q2/metrics.prom \
     --summary artifacts/nexus/load/payments-2026q2/slot_summary.json \
     --out-dir artifacts/nexus/load/payments-2026q2/slot_bundle \
     --metadata lane=payments \
     --metadata workload_seed=NX7-PAYMENTS-2026Q2
   ```

3. Export the lane-governance snapshot as JSON + Parquet for long-term audits:

   ```bash
   cargo xtask nexus-lane-audit \
     --status artifacts/nexus/load/payments-2026q2/torii_status.json \
     --json-out artifacts/nexus/load/payments-2026q2/lane_audit.json \
     --parquet-out artifacts/nexus/load/payments-2026q2/lane_audit.parquet \
     --captured-at 2026-05-10T10:15:00Z
   ```

   The JSON/Parquet snapshot now records TEU utilisation, scheduler trigger
   levels, RBC chunk/byte counters, and transaction graph statistics for each
   lane so the rollout evidence shows both backlog and execution pressure.

4. Run the smoke helper again at peak load so the thresholds are evaluated under stress (write the
   output to `smoke_during.log`) and re-run once the workload finishes (`smoke_after.log`).

### 6.3 Telemetry pack & governance manifest

The run directory must include a telemetry pack (`prometheus.tgz`, OTLP stream, structured logs, and
any workload harness output). Validate it and stamp the metadata that governance expects:

```bash
scripts/telemetry/validate_nexus_telemetry_pack.py \
  artifacts/nexus/load/payments-2026q2 \
  --manifest-out artifacts/nexus/load/payments-2026q2/telemetry_manifest.json \
  --expected prometheus.tgz --expected otlp.ndjson \
  --expected torii_structured_logs.jsonl --expected B4-RB-2026Q1.log \
  --slot-range 81200-81600 --require-slot-range \
  --workload-seed NX7-PAYMENTS-2026Q2 --require-workload-seed \
  --metadata lane=payments --metadata run=2026q2-rollout
```

Finally, attach the telemetry log captured earlier and require alias migration evidence when a lane
renames during the test:

```bash
scripts/nexus_lane_smoke.py \
  --status-file artifacts/nexus/load/payments-2026q2/torii_status.json \
  --metrics-file artifacts/nexus/load/payments-2026q2/metrics.prom \
  --telemetry-file artifacts/nexus/load/payments-2026q2/nexus.lane.topology.ndjson \
  --require-alias-migration core:payments \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-block-height 50000 \
  --max-finality-lag 4 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 200
```

Archive the following artefacts for the governance ticket:

- `smoke_before.log`, `smoke_during.log`, `smoke_after.log`
- `metrics.prom`, `slot_summary.json`, `slot_bundle_manifest.json`
- `lane_audit.{json,parquet}`
- `telemetry_manifest.json` + pack contents (`prometheus.tgz`, `otlp.ndjson`, etc.)
- `nexus.lane.topology.ndjson` (or the relevant telemetry slice)

The run can now be referenced inside Space Directory manifests and governance trackers as the
canonical NX-7 load test for the lane.

## 7. Telemetry & governance follow-ups

- Update the lane dashboards (`dashboards/grafana/nexus_lanes.json` and related overlays) with the
  new lane id and metadata. The generated metadata keys (`contact`, `channel`, `runbook`, etc.) make
  it simple to pre-fill labels.
- Wire PagerDuty/Alertmanager rules for the new lane before enabling admission. The `summary.json`
  next-steps array mirrors the checklist in `docs/source/nexus_operations.md`.
- Register the manifest bundle in the Space Directory once the validator set is live. Use the same
  manifest JSON generated by the helper, signed according to the governance runbook.
- Follow `docs/source/sora_nexus_operator_onboarding.md` for smoke tests (FindNetworkStatus, Torii
  reachability) and capture the evidence with the artefact set produced above.

## 8. Dry-run example

To preview the artefacts without writing files:

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator i105... \
  --validator i105... \
  --dry-run
```

The command prints the JSON summary and the TOML snippet to stdout, allowing quick iteration during
planning.

---

For additional context see:

- `docs/source/nexus_operations.md` — operational checklist and telemetry requirements.
- `docs/source/sora_nexus_operator_onboarding.md` — detailed onboarding flow that references the
  new helper.
- `docs/source/nexus_lanes.md` — lane geometry, slugs, and storage layout used by the tool.
