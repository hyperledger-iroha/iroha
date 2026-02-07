---
lang: hy
direction: ltr
source: docs/source/crypto/gost_performance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7fab384ae80e1993b1e54d6addc82fd3dc652fb6e3958bea6a04e057a1805b57
source_last_modified: "2025-12-29T18:16:35.939573+00:00"
translation_last_reviewed: 2026-02-07
---

# GOST Performance Workflow

This note documents how we track and enforce the performance envelope for the
TC26 GOST signing backend.

## Running locally

```bash
make gost-bench                     # run benches + tolerance check
make gost-bench GOST_BENCH_ARGS="--tolerance 0.30"  # override guard
make gost-dudect                    # run the constant-time timing guard
./scripts/update_gost_baseline.sh   # bench + rebaseline helper
```

Behind the scenes both targets call `scripts/gost_bench.sh`, which:

1. Executes `cargo bench -p iroha_crypto --bench gost_sign --features gost -- --noplot`.
2. Runs `gost_perf_check` against `target/criterion`, verifying medians against the
   checked-in baseline (`crates/iroha_crypto/benches/gost_perf_baseline.json`).
3. Injects the Markdown summary into `$GITHUB_STEP_SUMMARY` when available.

To refresh the baseline after approving a regression/improvement, run:

```bash
make gost-bench-update
```

or directly:

```bash
./scripts/gost_bench.sh --write-baseline \
  --baseline crates/iroha_crypto/benches/gost_perf_baseline.json
```

`scripts/update_gost_baseline.sh` runs the bench + checker, overwrites the baseline JSON, and prints
the new medians. Always commit the updated JSON alongside the decision record in
`crates/iroha_crypto/docs/gost_backend.md`.

### Current reference medians

| Algorithm            | Median (µs) |
|----------------------|-------------|
| ed25519              | 69.67       |
| gost256_paramset_a   | 1136.96     |
| gost256_paramset_b   | 1129.05     |
| gost256_paramset_c   | 1133.25     |
| gost512_paramset_a   | 8944.39     |
| gost512_paramset_b   | 8963.60     |
| secp256k1            | 160.53      |

## CI

`.github/workflows/gost-perf.yml` uses the same script and also runs the dudect timing guard.
CI fails when the measured median exceeds the baseline by more than the configured tolerance
(20% by default) or when the timing guard detects a leak, so regressions are caught automatically.

## Summary output

`gost_perf_check` prints the comparison table locally and appends the same content to
`$GITHUB_STEP_SUMMARY`, so CI job logs and run summaries share the same numbers.
