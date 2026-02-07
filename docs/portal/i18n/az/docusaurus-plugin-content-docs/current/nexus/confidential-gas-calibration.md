---
slug: /nexus/confidential-gas-calibration
lang: az
direction: ltr
source: docs/portal/docs/nexus/confidential-gas-calibration.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Confidential Gas Calibration Ledger
description: Release-quality measurements backing the confidential gas schedule.
---

# Confidential Gas Calibration Baselines

This ledger tracks the validated outputs of the confidential gas calibration
benchmarks. Each row documents a release-quality measurement set captured with
the procedure described in [Confidential Assets & ZK Transfers](./confidential-assets#calibration-baselines--acceptance-gates).

| Date (UTC) | Commit | Profile | `ns/op` | `gas/op` | `ns/gas` | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| 2025-10-18 | 3c70a7d3 | baseline-neon | 2.93e5 | 1.57e2 | 1.87e3 | Darwin 25.0.0 arm64e (hostinfo); `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`; `cargo test -p iroha_core bench_repro -- --ignored`; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`; `rustc 1.88.0 (6b00bc3)` |
| 2026-04-12 | pending | baseline-simd-neutral | — | — | — | Scheduled x86_64 neutral run on CI host `bench-x86-neon0`; see ticket GAS-214. Results will be added once the bench window completes (pre-merge checklist targets release 2.1). |
| 2026-04-13 | pending | baseline-avx2 | — | — | — | Follow-up AVX2 calibration using the same commit/build as the neutral run; requires host `bench-x86-avx2a`. GAS-214 covers both runs with delta comparison against `baseline-neon`. |

`ns/op` aggregates the median wall-clock per instruction measured by Criterion;
`gas/op` is the arithmetic mean of the corresponding schedule costs from
`iroha_core::gas::meter_instruction`; `ns/gas` divides the summed nanoseconds by
the summed gas across the nine-instruction sample set.

*Note.* The current arm64 host does not emit Criterion `raw.csv` summaries out of
the box; rerun with `CRITERION_OUTPUT_TO=csv` or an upstream fix before tagging a
release so the artefacts required by the acceptance checklist are attached.
If `target/criterion/` is still missing after `--save-baseline`, collect the run
on a Linux host or serialize the console output into the release bundle as a
temporary stop-gap. For reference, the arm64 console log from the latest run
lives at `docs/source/confidential_assets_calibration_neon_20251018.log`.

Per-instruction medians from the same run (`cargo bench -p iroha_core --bench isi_gas_calibration`):

| Instruction | median `ns/op` | schedule `gas` | `ns/gas` |
| --- | --- | --- | --- |
| RegisterDomain | 3.46e5 | 200 | 1.73e3 |
| RegisterAccount | 3.15e5 | 200 | 1.58e3 |
| RegisterAssetDef | 3.41e5 | 200 | 1.71e3 |
| SetAccountKV_small | 3.28e5 | 67 | 4.90e3 |
| GrantAccountRole | 3.33e5 | 96 | 3.47e3 |
| RevokeAccountRole | 3.12e5 | 96 | 3.25e3 |
| ExecuteTrigger_empty_args | 1.42e5 | 224 | 6.33e2 |
| MintAsset | 1.56e5 | 150 | 1.04e3 |
| TransferAsset | 3.68e5 | 180 | 2.04e3 |

The schedule column is enforced by `gas::tests::calibration_bench_gas_snapshot`
(total 1,413 gas across the nine-instruction set) and will trip if future patches
change metering without updating the calibration fixtures.
