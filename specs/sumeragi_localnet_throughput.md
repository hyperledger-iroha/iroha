# Sumeragi Localnet Throughput Harness

This document defines the 7-peer localnet throughput regression and the
1-second finality SLOs used during the first release. It covers the Kagami
perf profiles, deterministic load recipe, and the artifacts produced by the
integration harness.

## Perf Profiles (Kagami Localnet)

Use the dedicated profiles to sign a 1s block cadence and throughput bounds
into genesis while generating bounded peer configs:

- Permissioned: `kagami localnet --perf-profile 10k-permissioned`
- NPoS: `kagami localnet --perf-profile 10k-npos`

These profiles sign `block_cadence_ms = 1_000`, consensus mode, NPoS election
context where applicable, and the on-chain block limit of 10,000 transactions.
Peer configs retain the bounded runtime proposal cap of
`sumeragi.block.max_transactions = 256`, a 4 MiB localnet P2P frame cap, and
shorter transaction-gossip cadence (`transaction_gossip_period_ms = 100`,
`transaction_gossip_resend_ticks = 1`). No local collector, RBC, DA, or
pacemaker switch participates in the profile.

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
- On-chain block max transactions: 10_000
- Runtime proposal cap: 256
- Localnet P2P frame cap: 4 MiB
- Signed block cadence: 1000 ms

## Required Telemetry/Status Fields

### `/status`
- `blocks_non_empty`
- `queue_size`
- `txs_approved`
- `commit_time_ms`
- `sumeragi.tx_queue_depth`
- `sumeragi.tx_queue_capacity`
- `sumeragi.tx_queue_retained_bytes`
- `sumeragi.tx_queue_max_retained_bytes`
- `sumeragi.tx_queue_saturated`
- `sumeragi.tx_queue_saturated_by_count`
- `sumeragi.tx_queue_saturated_by_bytes`
- `sumeragi.tx_queue_saturated_by_age`
- `sumeragi.tx_queue_oldest_queued_age_ms`

### `/v1/sumeragi/status` (diagnostic projection)
- `view_change_install_total`
- `tx_queue_depth`
- `tx_queue_capacity`
- `tx_queue_retained_bytes`
- `tx_queue_max_retained_bytes`
- `tx_queue_saturated`
- `tx_queue_saturated_by_count`
- `tx_queue_saturated_by_bytes`
- `tx_queue_saturated_by_age`
- `tx_queue_oldest_queued_age_ms`
- `commit_qc.height`

### `/metrics` (Prometheus)
- `commit_time_ms_bucket`
- `commit_time_ms_sum`
- `commit_time_ms_count`

Use `/status` plus committed-height observations for pass/fail decisions. The
diagnostic projection supplements artifacts but does not override signed
consensus context.

## Running the Harness

Recommended wrapper:

```bash
scripts/run_localnet_throughput.sh --release --artifact-dir ./artifacts/localnet-throughput
```

For repeated local runs, add `--target-dir <dir>` to pin `CARGO_TARGET_DIR`.
When an `iroha3d` binary already exists under that target root, the wrapper now
reuses it and auto-sets `IROHA_TEST_SKIP_BUILD=1`; pass `--no-skip-build` to
force the legacy nested-build path.

Manual command:

```bash
IROHA_THROUGHPUT_ARTIFACT_DIR=./artifacts/localnet-throughput \
  cargo test -p integration_tests --release \
  --test consensus_and_da \
  sumeragi_localnet_smoke::permissioned_localnet_throughput_10k_tps \
  -- --ignored --exact --nocapture
```

NPoS run:

```bash
IROHA_THROUGHPUT_ARTIFACT_DIR=./artifacts/localnet-throughput \
  cargo test -p integration_tests --release \
  --test consensus_and_da \
  sumeragi_localnet_smoke::npos_localnet_throughput_10k_tps \
  -- --ignored --exact --nocapture
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
