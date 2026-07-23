---
title: Sumeragi V2 Multilane Scaling Gate
sidebar_label: Multilane Scaling Gate
description: Collection and validation contract for the five-pair one-lane versus four-lane G-SCALE release proof.
---

# Sumeragi V2 multilane scaling gate

This runbook defines the evidence consumed by `G-SCALE` in the
[Sumeragi V2 multilane closure ledger](sumeragi_v2_multilane_closure_ledger.md).
The checked-in runner and validator make the performance claim reproducible;
they do not make the claim true without fresh measurements. No benchmark
result, host identity, or passing evidence bundle is checked in, and running
the tooling does not update the ledger, `status.md`, or `roadmap.md`.

The entrypoint is:

```text
scripts/nexus/run_multilane_scaling_gate.sh
```

It runs exactly five pairs in this fixed order:

```text
pair 1: one_lane, four_lane
pair 2: one_lane, four_lane
...
pair 5: one_lane, four_lane
```

There is no run-count, skip, continue-on-failure, or threshold override. A pair
shares the SHA-256-derived workload seed, and every run uses the same exact
offered transaction count. Seeds are unique between pairs. The one-lane
observation must name exactly one active execution lane; the four-lane
observation must name exactly four, including the baseline lane. Lane sets
remain identical across all five trials for a variant.

## What the gate measures

Every trial has a declared warmup followed by one contiguous measurement
window. The raw JSON contains ordered intervals covering the window exactly.
Each interval records offered, accepted, and committed counts; one positive
commit-latency observation per committed transaction; and queue, index, memory,
and disk observations.

The four resource scopes are operator declarations supplied on the runner
command line. Use stable meanings for all ten trials:

- **Queue:** the maximum transaction queue depth reported by any localnet peer.
- **Index:** the lane/index entry count on one declared storage peer. Do not
  switch peers between trials.
- **Memory:** aggregate resident bytes for all peer processes in the localnet.
- **Disk:** aggregate bytes in the declared lane, Kura, and index storage roots.

The limits are fixed before collection. Every raw interval and recomputed
per-run maximum must remain within its declared limit. Missing values, booleans
masquerading as numbers, negative values, JSON `NaN`/`Infinity`, or a summary
that disagrees with its raw intervals fail validation.

The non-weakenable statistical contract is:

- at least 20 ordered measurement intervals per run;
- at least 100 per-transaction commit-latency samples per run;
- actual offered rate within 1% of the declared rate;
- identical offered count across all ten runs;
- median of the five four-lane committed-throughput results at least `1.5`
  times the median of the five one-lane results; and
- nearest-rank p95 over all five four-lane latency sample sets no greater than
  `1.25` times the corresponding pooled one-lane p95.

Committed throughput is recomputed as `committed_count /
measurement_seconds`. The validator never trusts a reported throughput or
percentile.

## Pin the identity and inputs

Use one otherwise idle host and one exact localnet configuration for the whole
matrix. The identity document has this shape; values below are field
descriptions, not evidence:

```json
{
  "schema": "iroha.sumeragi_v2.multilane.scaling.identity.v1",
  "hardware": {
    "machine_id": "<stable lab inventory identity>",
    "cpu_model": "<full CPU model>",
    "physical_core_count": "<positive integer>",
    "logical_core_count": "<positive integer>",
    "memory_bytes": "<positive integer>",
    "storage_model": "<device model and topology>"
  },
  "software": {
    "os": "<OS release>",
    "kernel": "<kernel release>",
    "architecture": "<architecture>",
    "python_version": "<python --version>",
    "rustc_version": "<rustc --version used for the binaries>",
    "source_revision": "<lowercase 40- or 64-hex commit>",
    "workspace_source_sha256": "<scripts/compute_workspace_source_manifest.py output>",
    "nexus_config_sha256": "<SHA-256 of the supplied configuration>",
    "irohad_sha256": "<SHA-256 of the release irohad binary>",
    "iroha_cli_sha256": "<SHA-256 of the release iroha CLI binary>"
  }
}
```

The trial harness must re-observe this identity immediately before and after
each measurement and put both observations in `identity_before` and
`identity_after`. The validator compares both objects structurally with the
pinned declaration on every run. Any hardware, OS, source, configuration, or
binary drift fails the bundle.

The runner copies and hashes the identity, configuration, trial harness,
validator, and these existing helpers into the evidence directory:

- `scripts/deploy_localnet.sh`;
- `scripts/tx_load.py`; and
- `scripts/nexus_lane_load_test.py`.

This lets the operator build the trial harness from the established localnet,
load generator, lifecycle/metrics smoke, and slot-bundle paths instead of
creating another deployment or load stack. The runner exposes their absolute
paths as `IROHA_GSCALE_DEPLOY_LOCALNET`, `IROHA_GSCALE_TX_LOAD`, and
`IROHA_GSCALE_NEXUS_LANE_LOAD_TEST`.

## Trial harness contract

Pass one executable, no-argument harness with `--trial-command`. The runner
invokes the same file for all ten trials from the repository root and captures
combined stdout/stderr. The harness must:

1. create a clean localnet from `IROHA_GSCALE_CONFIG_FILE`;
2. activate exactly `IROHA_GSCALE_ACTIVE_EXECUTION_LANES` execution lanes;
3. derive all workload choices from `IROHA_GSCALE_SEED`;
4. drive the target rate with the existing `tx_load.py` path;
5. collect lifecycle/metrics support with `nexus_lane_load_test.py`;
6. observe resources throughout the measurement window;
7. stop the localnet without leaving a run alive; and
8. write one strict JSON object to `IROHA_GSCALE_RAW_SAMPLES_OUT`.

The raw object hash-binds four run-specific support files: the
`nexus_lane_load_test.py` `load_test_manifest.json`, its lifecycle snapshot,
its Prometheus metrics snapshot, and the `tx_load.py` log. The validator
requires unique support files for every run and verifies that the Nexus
manifest's lane list and workload seed match the raw observation.

Important environment variables include:

| Variable | Meaning |
| --- | --- |
| `IROHA_GSCALE_PAIR_INDEX` | Decimal pair index `1` through `5` |
| `IROHA_GSCALE_VARIANT` | `one_lane` or `four_lane` |
| `IROHA_GSCALE_ACTIVE_EXECUTION_LANES` | Required active execution-lane count |
| `IROHA_GSCALE_SEED` | Pair-shared deterministic SHA-256 seed |
| `IROHA_GSCALE_OFFERED_LOAD_TPS` | Matched offered rate |
| `IROHA_GSCALE_WARMUP_SECONDS` | Warmup duration, excluded from samples |
| `IROHA_GSCALE_MEASUREMENT_SECONDS` | Exact sampled measurement duration |
| `IROHA_GSCALE_MIN_INTERVAL_SAMPLES` | Required interval floor |
| `IROHA_GSCALE_MIN_LATENCY_SAMPLES` | Required latency floor |
| `IROHA_GSCALE_MAX_*` | Queue/index/memory/disk limits |
| `IROHA_GSCALE_RUN_DIR` | Directory for run-specific support artifacts |
| `IROHA_GSCALE_IDENTITY_FILE` | Archived pinned identity |

Run `scripts/nexus/run_multilane_scaling_gate.sh --help` for the complete
interface. The raw file schema is enforced by
`validate_multilane_scaling_evidence.py`; use
`scripts/tests/run_multilane_scaling_gate_test.py` as a non-production contract
fixture when implementing a harness. Test fixture values must never be
submitted as release evidence.

## Run and validate

The values in this command are intentionally symbolic:

```bash
scripts/nexus/run_multilane_scaling_gate.sh \
  --artifact-dir "${NEW_G_SCALE_EVIDENCE_DIR}" \
  --identity-file "${PINNED_IDENTITY_JSON}" \
  --config-file "${PINNED_NEXUS_CONFIG}" \
  --trial-command "${G_SCALE_TRIAL_HARNESS}" \
  --seed-namespace "${RELEASE_SEED_NAMESPACE}" \
  --offered-load-tps "${MATCHED_TPS}" \
  --warmup-seconds "${WARMUP_SECONDS}" \
  --measurement-seconds "${MEASUREMENT_SECONDS}" \
  --max-queue-depth "${QUEUE_LIMIT}" \
  --max-index-entries "${INDEX_LIMIT}" \
  --max-memory-bytes "${RSS_LIMIT_BYTES}" \
  --max-disk-bytes "${DISK_LIMIT_BYTES}" \
  --queue-observation-scope "${QUEUE_SCOPE}" \
  --index-observation-scope "${INDEX_SCOPE}" \
  --memory-observation-scope "${MEMORY_SCOPE}" \
  --disk-observation-scope "${DISK_SCOPE}"
```

The artifact directory must not already exist. This prevents an old bundle or
partial retry from being overwritten. If a command fails, times out, omits raw
JSON, or an input changes during collection, the runner records the failure,
leaves later trials pending, invokes the validator, and exits nonzero.

The bundle contains:

```text
scaling_evidence.json
validation_report.json
inputs/
tooling/
runs/pair_01/one_lane/{raw_samples.json,trial.log,support/...}
runs/pair_01/four_lane/{raw_samples.json,trial.log,support/...}
...
```

An archived bundle can be checked again without running a benchmark:

```bash
python3 scripts/nexus/validate_multilane_scaling_evidence.py \
  "${G_SCALE_EVIDENCE_DIR}/scaling_evidence.json" \
  --report "${G_SCALE_EVIDENCE_DIR}/validation_report.recheck.json"
```

Validation fails for a missing, duplicate, or unordered pair; seed or offered
load mismatch; a skipped, failed, timed-out, or pending run; wrong active lane
count; identity or lane-set drift; weak samples; nonfinite or inconsistent
values; resource-limit breach; artifact hash/path violation; or either
performance threshold.

## Closure-ledger handoff

A `validation_report.json` with `"result": "pass"` is necessary but not by
itself sufficient to mark `G-SCALE` evidenced. Archive the complete directory
with the release record, bind its manifest/report hashes in that record, and
confirm the source and configuration identities belong to the candidate being
released. Only then may the closure-ledger owner change `G-SCALE` evidence
state. Never copy a test fixture, edit a failed report, or infer results from
human-readable logs.
