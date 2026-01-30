---
lang: fr
direction: ltr
source: docs/source/i3_bench_suite.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a3158cd70a42104bacaafc520fdcc10e20e3bc347d895be448fcb10da4f668bd
source_last_modified: "2026-01-03T18:08:01.692664+00:00"
translation_last_reviewed: 2026-01-30
---

# Iroha 3 Bench Suite

The Iroha 3 bench suite times the hot paths we rely on during staking, fee
charging, proof verification, scheduling, and proof endpoints. It runs as an
`xtask` command with deterministic fixtures (fixed seeds, fixed key material,
and stable request payloads) so results are reproducible across hosts.

## Running the suite

```bash
cargo xtask i3-bench-suite \
  --iterations 64 \
  --sample-count 5 \
  --json-out benchmarks/i3/latest.json \
  --csv-out benchmarks/i3/latest.csv \
  --markdown-out benchmarks/i3/latest.md \
  --threshold benchmarks/i3/thresholds.json \
  --allow-overwrite
```

Flags:

- `--iterations` controls iterations per scenario sample (default: 64).
- `--sample-count` repeats each scenario to compute the median (default: 5).
- `--json-out|--csv-out|--markdown-out` choose output artifacts (all optional).
- `--threshold` compares medians against the baseline bounds (set `--no-threshold`
  to skip).
- `--flamegraph-hint` annotates the Markdown report with the `cargo flamegraph`
  command to profile a scenario.

CI glue lives in `ci/i3_bench_suite.sh` and defaults to the paths above; set
`I3_BENCH_ITERATIONS`/`I3_BENCH_SAMPLES` to tune runtime in nightlies.

## Scenarios

- `fee_payer` / `fee_sponsor` / `fee_insufficient` — payer vs sponsor debit
  and shortfall rejection.
- `staking_bond` / `staking_slash` — bond/unbond queue with and without
  slashing.
- `commit_cert_verify` / `jdg_attestation_verify` / `bridge_proof_verify` —
  signature verification over commit certificates, JDG attestations, and bridge
  proof payloads.
- `commit_cert_assembly` — digest assembly for commit certificates.
- `access_scheduler` — conflict-aware access-set scheduling.
- `torii_proof_endpoint` — Axum proof endpoint parsing + verification round trip.

Every scenario records median nanoseconds per iteration, throughput, and a
deterministic allocation counter for quick regressions. Thresholds live in
`benchmarks/i3/thresholds.json`; bump bounds there when hardware changes and
commit the new artifact alongside a report.

## Troubleshooting

- Pin CPU frequency/governor when collecting evidence to avoid noisy regressions.
- Use `--no-threshold` for exploratory runs, then re-enable once the baseline is
  refreshed.
- To profile a single scenario, set `--iterations 1` and re-run under
  `cargo flamegraph -p xtask -- i3-bench-suite --iterations 128 --sample-count 1 --no-threshold --flamegraph-hint`.
