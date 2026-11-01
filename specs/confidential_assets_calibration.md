# Confidential Gas Calibration Baselines

This ledger retains historical confidential gas calibration measurements.
They do not qualify the current combined source or substitute for the named
reference platforms in `specs/confidential_assets.md#calibration-baselines--acceptance-gates`.
An Intel i7-11800H is now available for engineering instruction checks, but the
required Xeon Gold 6430 neutral/AVX2 baselines remain open. The Parliament Xeon
Silver 4310 baseline is a separate requirement. See
[Windows and Intel completion goals](windows_intel_completion_goals.md).

| Date (UTC) | Commit | Profile | `ns/op` | `gas/op` | `ns/gas` | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| 2025-10-18 | 3c70a7d3 | baseline-neon | 2.93e5 | 1.57e2 | 1.87e3 | Darwin 25.0.0 arm64e (hostinfo); `cargo bench -p iroha_core --features bench --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`; `cargo test -p iroha_core bench_repro -- --ignored`; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`; `rustc 1.88.0 (6b00bc3)` |
| 2026-04-28 | 8ea9b2a7 | baseline-neon-20260428 | 4.29e6 | 1.57e2 | 2.73e4 | Darwin 25.0.0 arm64 (`rustc 1.91.0`). Command: `cargo bench -p iroha_core --features bench --bench isi_gas_calibration -- --sample-size=10 --warm-up-time=2 --noplot --save-baseline baseline-neon-20260428`; log at `specs/confidential_assets_calibration_neon_20260428.log`. x86_64 parity runs (SIMD-neutral + AVX2) are scheduled for the 2026-03-19 Zurich lab slot; artefacts will land under `artifacts/confidential_assets_calibration/2026-03-x86/` with matching commands and will be merged into the baseline table once captured. |
| 2026-04-28 | — | baseline-simd-neutral | — | — | — | **Waived** on Apple Silicon—`ring` enforces NEON for the platform ABI, so `RUSTFLAGS="-C target-feature=-neon"` fails before the bench can run (`specs/confidential_assets_calibration_simd_neutral_attempt_20260428.log`). Neutral data stays gated on CI host `bench-x86-neon0`. |
| 2026-04-28 | — | baseline-avx2 | — | — | — | **Deferred** until an x86_64 runner is available. `arch -x86_64` cannot spawn binaries on this machine (“Bad CPU type in executable”; see `specs/confidential_assets_calibration_avx2_attempt_20260428.log`). CI host `bench-x86-avx2a` remains the source of record. |

`ns/op` aggregates the median wall-clock per instruction measured by Criterion;
`gas/op` is the arithmetic mean of the corresponding schedule costs from
`iroha_core::gas::meter_instruction`; `ns/gas` divides the summed nanoseconds by
the summed gas across the nine-instruction sample set.

The historical runs above did not retain the required Criterion `raw.csv`
artifacts. Current benchmark builds enable the workspace `csv_output` feature;
it is a compile-time reporter selection, not a `CRITERION_OUTPUT_TO` switch.
Fresh named-host runs must retain the original CSV samples under
`target/criterion/**/raw.csv` and follow the current [calibration
procedure](confidential_assets.md#calibration-baselines--acceptance-gates).
Console logs do not replace those samples or qualify a historical run. The
retained arm64 console log is
`specs/confidential_assets_calibration_neon_20251018.log`.

The current ISI benchmark covers eight instructions (the rows below except
`RegisterDomain`). Its `ISI_GAS_CALIBRATION` records report the measured fixture's
gas quote for one execution. The IVM benchmark emits `IVM_GAS_CALIBRATION`
records with operation count, actual total gas, HALT gas and per-operation gas;
its raw samples measure the complete repeated-instruction program. Keep these
records beside the original Criterion samples and derive current ratios from
them, rather than reusing the historical nine-instruction denominator below.
Benchmark IDs are unchanged. Neither emitted denominators nor a smoke run
qualify a named-host timing baseline.

The ISI target requires `--features bench,iroha-core-tests` to reuse the existing
signed four-validator genesis owner. Each case retains that original initialized
State and creates a fresh transaction against its authenticated parent for every
iteration; dropping the uncommitted transaction rolls back its changes. Each
measured transaction opens the existing bounded component invocation owner before
any callback executes; that fixture owner grants no Network or block-publication
authority. The callback regression consumes a one-shot empty trigger and checks
that discarding the overlay restores it. Genesis startup and case preconditions
stay outside timing. The `bench_repro` controls
check retained genesis/network/root authority and successful account and
domain-owned asset-definition registration followed by rollback, so a
header-shaped pre-genesis fixture cannot qualify it. The asset's explicit owning
domain authorizes post-genesis registration; deriving its identifier from a
domain name alone supplies no ownership authority.
Role cases retain the authority's own membership and grant or revoke the role
on a distinct recipient; role-management permission alone does not authorize
delegating an unheld role. The regression executes both operations and checks
that dropping each transaction restores the recipient's original membership.

The neutral profile's `-C target-feature=-avx,-avx2,-fma` flags alone do not
establish scalar runtime execution. CPU detection can still select explicitly
annotated kernels; for example, `iroha_fhe::active_backend()` selects AVX2 when
the `simd` feature and CPU support are present. Retain the selected backend and
the effective policy for every exercised acceleration owner with each
qualification run; unavailable dispatch evidence leaves the neutral gate open.
Both benchmarks accept the benchmark-only setting
`IROHA_GAS_BENCH_IVM_POLICY=scalar`, which applies IVM's existing
`AccelerationConfig` with SIMD, Metal and CUDA disabled and asserts its selected
`Scalar` backend. The default `runtime` preserves the existing policy; unknown
values refuse. `GAS_CALIBRATION_IVM_POLICY scope=ivm-only` records the requested
policy, actual selection and effective backend enables. For example:

```sh
IROHA_GAS_BENCH_IVM_POLICY=scalar IROHA_CONF_GAS_SEED=conf-gas-seed-2026Q1 \
  cargo bench --locked -p ivm --bench gas_calibration -- --test
```

This is a runnable smoke control, not a named-host timing measurement. The IVM
benchmark exercises arithmetic programs and numeric/TLV operations, including
their VM memory machinery. The ISI benchmark also generates Ed25519 accounts
and uses model/Core hashing and execution owners outside IVM. An IVM policy
receipt is neither their dispatch evidence nor an instruction-completion count.
Neither benchmark loads node configuration; this setting exists only in their
shared benchmark helper, and does not control FHE or third-party dispatch. The
isolated `iroha_fhe --no-default-features` test suite proves only that component's
scalar build; Cargo feature unification must be checked in a complete consumer.
No benchmark command currently proves a completely scalar dependency graph.

Current Linux component validation completes all eight ISI smoke cases under
both `runtime` and `scalar`, with identical gas quotes, and passes all five
`bench_repro` controls. The IVM smokes complete all 37 cases under both policies.
These are execution and fixture controls on the development host, not the
required Xeon neutral/AVX2 baselines; see the
[Intel validation record](../docs/validation/intel_avx2.md).

Per-instruction medians from the same run (`cargo bench -p iroha_core --features bench --bench isi_gas_calibration`):

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

### 2026-04-28 (Apple Silicon, NEON enabled)

Median latencies for the 2026-04-28 refresh (`cargo bench -p iroha_core --features bench --bench isi_gas_calibration -- --sample-size=10 --warm-up-time=2 --noplot --save-baseline baseline-neon-20260428`):

| Instruction | median `ns/op` | schedule `gas` | `ns/gas` |
| --- | --- | --- | --- |
| RegisterDomain | 8.58e6 | 200 | 4.29e4 |
| RegisterAccount | 4.40e6 | 200 | 2.20e4 |
| RegisterAssetDef | 4.23e6 | 200 | 2.12e4 |
| SetAccountKV_small | 3.79e6 | 67 | 5.66e4 |
| GrantAccountRole | 3.60e6 | 96 | 3.75e4 |
| RevokeAccountRole | 3.76e6 | 96 | 3.92e4 |
| ExecuteTrigger_empty_args | 2.71e6 | 224 | 1.21e4 |
| MintAsset | 3.92e6 | 150 | 2.61e4 |
| TransferAsset | 3.59e6 | 180 | 1.99e4 |

`ns/op` and `ns/gas` aggregates in the table above are derived from the sum of
these medians (total `3.85717e7` ns across the nine-instruction set and 1,413
gas units).

The 1,413-gas total is historical. The current
`gas::tests::calibration_bench_gas_snapshot` checks the current instruction quotes,
including `RegisterDomain`, and derives their sum. It does not establish that
the eight-case benchmark still has this historical denominator; use the emitted
current quotes for a new calibration report.

## Commitment Tree Telemetry Evidence (M2.2)

Per roadmap task **M2.2**, every calibration run must capture the new
commitment-tree gauges and eviction counters to prove the Merkle frontier stays
within configured bounds:

- `iroha_confidential_tree_commitments{asset_id}`
- `iroha_confidential_tree_depth{asset_id}`
- `iroha_confidential_root_history_entries{asset_id}`
- `iroha_confidential_frontier_checkpoints{asset_id}`
- `iroha_confidential_frontier_last_checkpoint_height{asset_id}`
- `iroha_confidential_frontier_last_checkpoint_commitments{asset_id}`
- `iroha_confidential_root_evictions_total{asset_id}`
- `iroha_confidential_frontier_evictions_total{asset_id}`

Record the values immediately before and after the calibration workload. A
single command per asset is sufficient; example for `4cuvDVPuLBKJyN6dPbRQhmLh68sU`:

```bash
curl -s http://127.0.0.1:8180/metrics \
  | rg 'iroha_confidential_(tree_(commitments|depth)|root_history_entries|frontier_(checkpoints|last_checkpoint_height|last_checkpoint_commitments)|root_evictions_total|frontier_evictions_total){asset_id="4cuvDVPuLBKJyN6dPbRQhmLh68sU"}'
```

Attach the raw output (or Prometheus snapshot) to the calibration ticket so the
governance reviewer can confirm root-history caps and checkpoint intervals are
honoured. The telemetry guide in `specs/telemetry.md#confidential-tree-telemetry-m22`
expands on alerting expectations and the associated Grafana panels.

## Neutral & AVX2 Waiver

SDK Council granted a temporary waiver for the Phase C gate requiring
`baseline-simd-neutral` and `baseline-avx2` measurements:

- **SIMD-neutral:** On Apple Silicon the `ring` crypto backend enforces NEON for
  ABI correctness. Disabling the feature (`RUSTFLAGS="-C target-feature=-neon"`)
  aborts the build before the bench binary is produced (`specs/confidential_assets_calibration_simd_neutral_attempt_20260428.log`).
- **AVX2:** The local toolchain cannot spawn x86_64 binaries (`arch -x86_64 rustc -V`
  → “Bad CPU type in executable”; see
  `specs/confidential_assets_calibration_avx2_attempt_20260428.log`).

The waiver records the earlier Apple-host limitation. Current release
qualification still requires the named neutral/AVX2 measurements, canonical
Criterion artifacts and the other reference-platform/telemetry evidence above.
The available i7 instruction checks do not complete these timing baselines or
change committed gas costs.
