---
lang: ka
direction: ltr
source: docs/source/sumeragi_localnet_throughput.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8385d3b46a19fcffc1a810be2ae0be4e17de67db63deb4415621368d528b6cb0
source_last_modified: "2026-01-28T04:31:10.014436+00:00"
translation_last_reviewed: 2026-02-07
---

# Sumeragi Localnet Throughput Harness

This document defines the 7-peer localnet throughput regression and the
1-second finality SLOs used during the first release. It covers the Kagami
perf profiles, deterministic load recipe, and the artifacts produced by the
integration harness.

## Perf Profiles (Kagami Localnet)

Use the dedicated profiles to stamp 1s block/commit defaults and throughput
caps into the generated configs:

- Permissioned: `kagami localnet --perf-profile 10k-permissioned`
- NPoS: `kagami localnet --perf-profile 10k-npos`

These profiles set 1s block/commit timing, `collectors_k`, `redundant_send_r`,
`block_max_transactions = 10_000`, shorter transaction gossip cadence
(`transaction_gossip_period_ms = 100`, `transaction_gossip_resend_ticks = 1`),
and NPoS bootstrap stake. Explicit CLI flags still override individual values.

## 1s Finality SLO Thresholds

Targets are evaluated during the steady-state window. View-change and
backpressure rates are per-peer maxima.

| Mode | Commit p95 (ms) | Commit p99 (ms) | View-change rate (per sec) | Backpressure rate (per sec) | Queue saturation fraction |
| ---- | -------------- | -------------- | -------------------------- | --------------------------- | ------------------------- |
| Permissioned | <= 1500 | <= 2000 | <= 0.10 | <= 2.0 | <= 0.20 |
| NPoS | <= 2000 | <= 3000 | <= 0.20 | <= 3.0 | <= 0.30 |

Override via environment variables:
`IROHA_THROUGHPUT_SLO_P95_MS`, `IROHA_THROUGHPUT_SLO_P99_MS`,
`IROHA_THROUGHPUT_SLO_VIEW_CHANGE_RATE`, `IROHA_THROUGHPUT_SLO_BACKPRESSURE_RATE`,
`IROHA_THROUGHPUT_SLO_QUEUE_SAT_FRAC`.

## Measurement Windows

- Warmup blocks: 10 (default)
- Steady-state blocks: 30 (default)
- Stall timeout (no height advance): 60s
- Sample interval: 2s

Override with `IROHA_THROUGHPUT_WARMUP_BLOCKS`, `IROHA_THROUGHPUT_STEADY_BLOCKS`,
and `IROHA_THROUGHPUT_TARGET_BLOCKS`.

## Deterministic Load Recipe

Defaults used by the integration test:

- Peers: 7
- Tx type: `Log` instruction (INFO)
- Payload size: 512 bytes (`IROHA_THROUGHPUT_PAYLOAD_BYTES`)
- RNG seed: `0x49524f4841` (`IROHA_THROUGHPUT_RNG_SEED`)
- Submit batch: 512 (`IROHA_THROUGHPUT_SUBMIT_BATCH`)
- Submit parallelism: 128 (`IROHA_THROUGHPUT_PARALLELISM`)
- Queue soft limit: 20_000 (`IROHA_THROUGHPUT_QUEUE_SOFT_LIMIT`)
- Block max transactions: 10_000
- Block/commit time: 1000 ms

## Required Telemetry/Status Fields

### `/status`
- `blocks_non_empty`
- `queue_size`
- `txs_approved`
- `commit_time_ms`
- `sumeragi.tx_queue_depth`
- `sumeragi.tx_queue_saturated`

### `/v1/sumeragi/status`
- `view_change_install_total`
- `pacemaker_backpressure_deferrals_total`
- `tx_queue_depth`
- `tx_queue_capacity`
- `tx_queue_saturated`
- `commit_qc.height`

### `/metrics` (Prometheus)
- `commit_time_ms_bucket`
- `commit_time_ms_sum`
- `commit_time_ms_count`

Gaps to add: none (current fields cover all throughput SLO metrics).

## Running the Harness

Recommended wrapper:

```bash
scripts/run_localnet_throughput.sh --release --artifact-dir ./artifacts/localnet-throughput
```

Manual command:

```bash
IROHA_THROUGHPUT_ARTIFACT_DIR=./artifacts/localnet-throughput \
  cargo test -p integration_tests --release \
  --test sumeragi_localnet_smoke permissioned_localnet_throughput_10k_tps \
  -- --ignored --nocapture
```

NPoS run:

```bash
IROHA_THROUGHPUT_ARTIFACT_DIR=./artifacts/localnet-throughput \
  cargo test -p integration_tests --release \
  --test sumeragi_localnet_smoke npos_localnet_throughput_10k_tps \
  -- --ignored --nocapture
```

## Artifacts

When `IROHA_THROUGHPUT_ARTIFACT_DIR` is set, each run writes:

- `summary.json` (run metadata + computed metrics)
- `status_samples.json` (per-sample status/sumeragi snapshots)
- `metrics/*.prom` (raw Prometheus snapshots, warmup + steady)

## Report Template

Use this template for each run:

```
Run Metadata
- Run ID:
- Timestamp:
- Git SHA:
- Hardware (CPU/cores/RAM):
- OS:
- Command:
- Config fingerprint:
- Artifact dir:
- Metrics dir:
- Peer log paths:

Recipe
- Mode:
- Peers:
- Block/commit time:
- Block max tx:
- Payload bytes + RNG seed:
- Submit batch/parallelism:
- Queue soft limit:
- Warmup/steady blocks:

Results
- Submitted TPS:
- Committed TPS:
- Commit p95/p99 (ms):
- View-change rate (avg/max):
- Backpressure rate (avg/max):
- Queue saturation fraction:
- Notes:
```
