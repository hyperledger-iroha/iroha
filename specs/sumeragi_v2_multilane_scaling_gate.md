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

Every trial has a fixed open-loop warmup, a bounded warmup drain, one contiguous
measurement window, and a bounded postmeasurement drain. All phases use one
monotonic clock whose origin is measurement start. The mandatory transaction
trace identifies every scheduled warmup and measurement request, its actual
submission time, its admission response, and its authoritative completion.
There is one strict first-release V1 schema; aggregate-only evidence is invalid.

The raw JSON contains ordered intervals covering measurement and drain exactly.
Each interval records offered, accepted, and committed counts; one positive
offer-to-observed-StateApplied latency per committed transaction; and queue,
index, memory, and disk observations. `accepted_count` counts observed Accepted
admission responses; `committed_count` counts exact global `Applied` responses
resolved from `state`. SDK submission success, an estimated aggregate count,
`Committed`, or cached `Applied` is insufficient. Observation latency includes
client polling and transport delay; it is not an inferred server commit time.

Events remain in the intervals in which they were observed. A transaction
offered or acknowledged in an earlier interval can become Applied later, and a
state observation can precede a delayed admission response. There is no
per-interval `committed <= accepted <= offered` constraint. The measurement
summary retains that constraint; an observation race that violates it makes a
run unqualified. Never move, clamp, or backdate observations to make it pass.
Drain contains no offers. Accepted responses received during drain count there;
late rejection responses remain explicit trace rows and are counted separately
in the validation report.

The four resource scopes are operator declarations supplied on the runner
command line. Use stable meanings for all ten trials:

- **Queue:** the maximum transaction queue depth reported by any localnet peer.
- **Index:** the lane/index entry count on one declared storage peer. Do not
  switch peers between trials.
- **Memory:** aggregate resident bytes for all peer processes in the localnet.
- **Disk:** aggregate bytes in the declared lane, Kura, and index storage roots.

The limits are fixed before collection. Every measurement and drain interval
and recomputed per-run maximum must remain within its declared limit. Drain
observation intervals cannot exceed the longest measurement interval. Missing values, booleans
masquerading as numbers, negative values, JSON `NaN`/`Infinity`, or a summary
that disagrees with its raw intervals fail validation.

The non-weakenable statistical contract is:

- at least 20 ordered measurement intervals per run;
- at least 100 in-window per-transaction StateApplied latency samples per run;
- actual offered rate within 1% of the declared rate;
- identical offered count across all ten runs;
- median of the five four-lane committed-throughput results at least `1.5`
  times the median of the five one-lane results; and
- nearest-rank p95 over the **complete accepted measurement cohorts**, including
  drain completions, from all five four-lane runs no greater than `1.25` times
  the corresponding pooled one-lane p95.

Committed throughput is recomputed as `committed_count /
measurement_seconds`, using only in-window measurement-cohort StateApplied
observations. Drain never increases throughput or its denominator. Every
accepted measurement request must have an exact StateApplied observation by the
fixed drain deadline; timeout, missing outcome, state rejection, or expiration
fails the run. Definitive admission rejections are recorded as explicit
`Rejected` responses with a reason and no Applied observation. Transport errors,
provisional queue outcomes, and unknown admission outcomes cannot be relabeled
as rejection. No pending or slow accepted request may be dropped from p95.
Warmup requests and their latency never enter either metric.

## Fixed schedule and transaction trace

The workload pins `offered_load_tps = R`, `warmup_seconds = W`,
`measurement_seconds = T`, `drain_seconds = D`, and
`max_submission_lag_ms = L` for all ten runs, alongside the existing statistical
floors. `T > 0`, `W >= 0`, and `0 < D <= 300`. Phase durations and `L` must
represent exact integer nanoseconds. `L` is nonnegative and at most one quarter
of the arrival period, `1000 / R` milliseconds. A missed schedule or exceeded
bound makes the trial unqualified; no backpressure rescheduling, missing
scheduled requests, or catch-up burst can repair it.

The schedule uses exact rational arithmetic over the decimal rate. For each
cohort, sequence numbers start at 1 and the scheduled offset is
`phase_start_ns + floor((sequence - 1) * 1_000_000_000 / R)`:

- Warmup has exactly `ceil(R * W)` offers in `[-(W + D), -D)` seconds. Every
  warmup acknowledgment and every accepted warmup StateApplied must precede 0.
  The interval `[-D, 0)` is its fixed drain. Zero warmup means zero warmup rows.
- Measurement has exactly `ceil(R * T)` offers in `[0, T)`. Every response and
  accepted StateApplied must arrive by `T + D`, inclusive. Observations at `T`
  belong to drain; the final drain interval includes its exact deadline.

`offer_offset_ns` records when the prepared signed transaction is actually
passed to the submission transport. Preparation and signing must not move the
schedule. `submission_lag_ns` must equal actual minus scheduled offset and lie
in `[0, L * 1_000_000]`. Adjacent permitted slots do not overlap, so a delayed
request cannot be submitted in a later slot alongside that slot's request.

The hash-bound `artifacts.transaction_trace` file is one strict JSON object:

| Field | Required value |
| --- | --- |
| `schema` | `iroha.sumeragi_v2.multilane_scaling.trace.v1` |
| `pair_index`, `variant`, `seed` | Exact values from the run |
| `clock` | `monotonic_nanoseconds_relative_to_measurement_start` |
| `logical_id_derivation` | `sha256(seed + ':' + cohort + ':' + decimal_sequence)` |
| `transaction_hash_source` | `iroha_data_model::transaction::SignedTransaction::hash` |
| `transactions` | All warmup rows followed by all measurement rows, each in sequence order |

Each transaction row has exactly these fields:

| Field | Meaning |
| --- | --- |
| `cohort` | `warmup` or `measurement` |
| `sequence` | One-based integer within its cohort |
| `logical_id` | Lowercase SHA-256 from the declared derivation, using decimal sequence without padding |
| `hash` | Canonical SDK signed transaction hash observed before submission |
| `scheduled_offset_ns`, `offer_offset_ns`, `submission_lag_ns` | Signed integer nanoseconds on the declared clock |
| `acknowledgment` | Exact admission-response projection below; never missing |
| `applied` | Exact state-response projection for Accepted; null for Rejected |

Logical IDs are paired workload identities: corresponding one- and four-lane
requests share their seed/cohort/sequence. Signed transaction hashes identify
the actual submissions and must be unique throughout the matrix, including
warmup. The collector must preserve their mapping while building and submitting
transactions. Hash text follows the existing SDK owner: exactly 64 lowercase
hexadecimal digits with the Iroha low-bit marker set (odd final hex digit).
The validator checks identity consistency; it does not independently decode
signed Norito transactions or prove that the archived harness emitted them.

`acknowledgment` has exactly `offset_ns`, `hash`, `status`, and `rejection`.
`status` is `Accepted` with null `rejection`, or definitive admission `Rejected`
with a nonempty reason. The hash must match the row, and its time cannot precede
the offer. `applied` has exactly `offset_ns`, `hash`, `scope`, `resolved_from`,
`status`, and `block_height`. It must name the same hash, `scope = global`,
`resolved_from = state`, `status = Applied`, and a positive u64 height. Its
observation time must strictly follow the offer; it need not follow the
admission response. These fields project the existing Torii
`PipelineTransactionStatusResponse` and the SDK's state-resolved wait contract.

Raw runs add `warmup`, containing exact offered/accepted/committed cohort counts,
and `drain`, containing `summary` and `samples` with the same strict field sets
as measurement. Every interval count and latency is recomputed from trace
events. Latency arrays are ordered by StateApplied observation offset, then
cohort sequence, and equal `(applied_offset_ns - offer_offset_ns) / 1_000_000`.
Both measurement and drain summaries equal their own raw intervals; the
reported resource maxima cover both phases. Complete-cohort latency and drain
accepted/committed/rejected counts are derived from rows, not supplied totals.

The validator rejects unknown fields, missing rows, duplicate identities,
reordered sequences, missing drain coverage, and unknown statuses. A run's
scheduled warmup plus measurement row count is bounded at 1,000,000 before
trace processing. Existing bundle limits remain: 256 regular files, 256 MiB
per file, 2 GiB total, with no symlinks, hard-link aliases, or unreferenced
artifacts. Hitting a bound fails collection; do not truncate or sample rows.

## Pin the identity and inputs

Use one otherwise idle host and one exact localnet configuration for the whole
matrix. The identity document has this shape; values below are field
descriptions, not evidence:

```json
{
  "schema": "iroha.sumeragi_v2.multilane_scaling.identity.v1",
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
    "irohad_sha256": "<SHA-256 of the release iroha3d binary>",
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

For release evidence, `software.workspace_source_sha256` must be the
permission-aware manifest of the retained sealed release tree, not a digest of
an earlier mutable checkout. The archived validator and all three archived
helpers must be byte-identical to the files in that retained tree. The release
receipt rechecks those files before and after replaying the validator.

## Trial harness contract

Pass one executable, no-argument harness with `--trial-command`. The runner
invokes the same file for all ten trials from the repository root and captures
combined stdout/stderr. The harness must:

1. create a clean localnet from `IROHA_GSCALE_CONFIG_FILE`;
2. activate exactly `IROHA_GSCALE_ACTIVE_EXECUTION_LANES` execution lanes;
3. derive all workload choices from `IROHA_GSCALE_SEED`;
4. drive the fixed open-loop schedule through the established load path,
   retaining exact per-transaction observations rather than aggregate estimates;
5. collect lifecycle/metrics support with `nexus_lane_load_test.py`;
6. fully drain warmup before measurement, observe resources throughout
   measurement and the fixed postwindow drain, and close every scheduled row;
7. stop the localnet without leaving a run alive; and
8. write one strict JSON object to `IROHA_GSCALE_RAW_SAMPLES_OUT`.

The raw object hash-binds five run-specific support files: the
`nexus_lane_load_test.py` `load_test_manifest.json`, its lifecycle snapshot,
its Prometheus metrics snapshot, the `tx_load.py` log, and the transaction trace. The validator
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
| `IROHA_GSCALE_DRAIN_SECONDS` | Fixed warmup and postmeasurement drain bound, at most 300 seconds |
| `IROHA_GSCALE_MAX_SUBMISSION_LAG_MS` | Fixed schedule lag bound, at most one quarter of an arrival period |
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
  --drain-seconds "${DRAIN_SECONDS}" \
  --max-submission-lag-ms "${MAX_SUBMISSION_LAG_MS}" \
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
mkdir -p "${RECHECK_OUTPUT_DIR}"
python3 scripts/nexus/validate_multilane_scaling_evidence.py \
  "${G_SCALE_EVIDENCE_DIR}/scaling_evidence.json" \
  --report "${RECHECK_OUTPUT_DIR}/validation_report.json"
```

The report parent must already exist and the destination must be absent.
Publication is durable and no-clobber: a regular file, symlink, or concurrent
publisher at the destination causes failure instead of replacement. Keep the
recheck report outside the archived bundle: unexpected files inside the bundle
are rejected.

Validation fails for a missing, duplicate, or unordered pair; seed or offered
load mismatch; a skipped, failed, timed-out, or pending run; wrong active lane
count; identity or lane-set drift; weak samples; nonfinite or inconsistent
values; resource-limit breach; artifact hash/path violation; or either
performance threshold.

The production per-transaction collector is still outstanding. The existing
`tx_load.py` aggregate counters cannot populate this trace or establish p95.
The routing/topology choice also remains open: manually activating static lanes
does not establish the autoscale routing corridor, and queue/index resource
observations need their production owners identified. The current gate keeps
one identical configuration hash for the whole matrix; it does not accept an
unreviewed paired-configuration scheme. These schema tests qualify the evidence
contract only, not a workload, deployment, resource collector, or scaling result.

## Closure-ledger handoff

A `validation_report.json` with `"result": "pass"` is necessary but not by
itself sufficient to mark `G-SCALE` evidenced. The production release must
receive these exact values through authenticated bootstrap
`--runner-environment NAME=VALUE` entries:

| Name | Required value |
| --- | --- |
| `IROHA_RELEASE_SCALING_EVIDENCE_MANIFEST` | Absolute canonical non-symlink path to the bundle's `scaling_evidence.json` |
| `IROHA_RELEASE_SCALING_TRIAL_HARNESS_SHA256` | Lowercase SHA-256 of the approved archived harness |
| `IROHA_RELEASE_SCALING_CONFIGURATION_SHA256` | Lowercase SHA-256 of the pinned archived configuration |
| `IROHA_RELEASE_SCALING_IROHAD_SHA256` | Lowercase SHA-256 named for the measured `iroha3d` binary |
| `IROHA_RELEASE_SCALING_IROHA_CLI_SHA256` | Lowercase SHA-256 named for the measured CLI binary |

Ambient or lookalike variables are not accepted. The sealed corridor checks
the four digests at every identity checkpoint, validates the bundle against
the sealed commit/workspace and retained validator/tooling before network
gates, and passes the same authenticated values to aggregate-receipt replay.
The receipt archives the complete bundle and those trust anchors.

Only after that receipt succeeds may the closure-ledger owner change
`G-SCALE` evidence state. Never copy a test fixture, edit a failed report, or
infer results from human-readable logs.
