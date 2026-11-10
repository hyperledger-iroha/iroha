# zk-spartan on the Pixel 6 (Claude b04a57, 2026-11-08)

Historical research record. See [README.md](README.md) for the current scope, reproduction commands and unresolved production gaps.

This folder's crate, unchanged, built with `cargo build --release --offline --target aarch64-linux-android`
(NDK 28.0.12674087, API 26 clang; lane target/cargo-fast/claude-zks-android; binary sha256
259ec5f5...fbd4). Pixel 6 19181FDF600918: 8 threads, 33.8 °C before and after, adb shell only, one
process per mode, oom_score_adj 1000. Log: ../../output/kagemusha-100x-20261108/spike/runs/zks-pixel6-8t.log; script: ../../output/kagemusha-100x-20261108/spike/runs/run-zks-pixel6.sh.

## Tamper suite: all 8 curve and round-size combinations correct (44.0 s; 4.4 s on the Mac)

For each curve and k = 4, 3, 2, 1 (k = 5 is skipped by the suite for these shapes):
- the honest proof is accepted;
- every single-bit flip, truncation, wrong io and the 3 unsatisfied witnesses are rejected;
- CancelRound and AdaptiveWitness are rejected (the unseparated aggregate still ACCEPTS both, as recorded).

Peak RSS 9.5 MB.

## Bench (peak RSS 285 MB for the whole run)

| shape | proof | prove | commit | outer | inner | open (IPA) | verify | Mac prove (16 threads) |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| send-primary, vesta 2^20, 8 x 65,536 | 1,824 B | 2.207 s | 0.378 | 0.288 | 0.214 | 1.328 | 0.203 s | 0.370 s |
| send-secondary, pallas 2^16, 8 x 8,192 | 1,600 B | 0.457 s | 0.074 | 0.034 | 0.042 | 0.306 | 0.086 s | 0.083 s |
| receive-primary, vesta 2^21, 16 x 65,536 | 2,144 B | 2.958 s | 0.708 | 0.513 | 0.385 | 1.353 | 0.200 s | 0.494 s |
| receive-secondary, pallas 2^17, 16 x 8,192 | 1,888 B | 0.524 s | 0.128 | 0.057 | 0.068 | 0.272 | 0.064 s | 0.117 s |
| send-primary-64x8k, k = 4 | 3,424 B | 1.392 s | 0.595 | 0.265 | 0.237 | 0.295 | 0.072 s | 0.297 s |
| send-primary-64x8k, k = 2 | 3,744 B | 1.346 s | 0.622 | 0.221 | 0.206 | 0.298 | 0.055 s | 0.289 s |
| receive-primary-128x8k, k = 4 | 5,536 B | 2.243 s | 1.052 | 0.487 | 0.422 | 0.282 | 0.054 s | 0.510 s |

Pairs (proof bytes measured; header and inherited claims as in research/REPORT4.md): Send 64x8k 6,048 B (spare 2,410
of 8,458), Send 64x8k k = 2 6,368 B, Receive 128x8k 8,448 B (spare 1,152 of 9,600).

## Against the gates

- Own IPA <= 0.4 s at 8,192 columns: **passes**, 0.282-0.298 s on Vesta and 0.272-0.306 s on Pallas.
- Primary prove core <= 1.5 s: **passes for Send at 64 x 8k** (1.35-1.39 s). It fails at 8 x 65,536 (2.21 s,
  the 65,536 IPA alone 1.33 s) and for Receive at 128 x 8k (2.24 s, which is commitment-bound: 1.05 s).
- Best-shape prover cores per operation:
  - Send: 64 x 8k primary + secondary = 1.35 + 0.46 = **~1.8 s**, verify ~0.16 s.
  - Receive: 128 x 8k + secondary = 2.24 + 0.52 = **~2.8 s**, verify ~0.12 s.

These are prover cores on the spike's synthetic R1CS. Not included: real witness generation, inherited-claim
handling, the preparation proof, native decisions and durable completion. On the shorter shapes, Receive's
commit phase (1.05 s) is the largest line.

Correction (3077ab): the spike's commit phase already uses public width classes (64/16/80/1-bit rows plus
two full-width masking columns), not full-width values. So the pay-per-bit saving is already partly in
these numbers; how much more remains depends on the real witness census. An earlier version of this file
said full-width.

## Commit-cost calibration (`commit-classes`, three runs, 2026-11-08 11:00Z)

Binary rebuilt from current source: source fingerprint c935dbbf… (sha256 of `shasum -a 256 Cargo.toml Cargo.lock
src/*.rs`), iroha 7f788ce30f with iroha_pasta clean, binary sha256 55440e6f…. 8 threads, battery 31.9 °C before
and after every run, 13.5-13.8 s per run, peak RSS 40 MB. Logs: `../../output/kagemusha-100x-20261108/spike/runs/zks-commit-classes-pixel6-run{1,2,3}.log`.

Median µs per entry (8 rows per cell). Full-width cells agree within 5% across runs; some narrow cells vary by
up to ±30%.

| columns | 1b | 8b | 13b | 16b | 32b | 64b | 88b | 128b | 180b | 255b |
|---|---|---|---|---|---|---|---|---|---|---|
| 8,192 | 0.209 | 0.401 | 0.755 | 0.607 | 1.057 | 1.624 | 2.204 | 2.519 | 3.383 | 4.473 |
| 16,384 | 0.208 | 0.446 | 1.119 | 0.574 | 1.242 | 2.239 | 1.822 | 1.930 | 1.893 | 3.759 |
| 32,768 | 0.228 | 0.474 | 0.848 | 0.463 | 0.700 | 1.364 | 1.308 | 1.980 | 1.429 | 2.399 |
| 65,536 | 0.161 | 0.344 | 0.468 | 0.371 | 0.536 | 1.112 | 1.199 | 1.535 | 1.550 | 2.196 |

Against `commit_us` at 16,384 columns:
- full width is 1.56x the model (3.76 vs 2.41 µs);
- 13-bit chunks are 1.81x (1.12 vs 0.62);
- 1-bit rows are 0.71x;
- 8- and 16-bit rows are within ±20%.

At 65,536 columns, full width is 1.08x (2.20 vs 2.04). **The gate (full width and 13-bit within 25% of the
model) fails at the fixed point's width.**

### Rescore with the measured table

`../../output/kagemusha-100x-20261108/b04a57-commit/` is a copy of this crate whose `commit_us` reads the table above when `ZKS_COMMIT=measured`:
linear in bits between measured widths, with 4,096 columns scaled from 8,192 by the model's ratio. Without the
variable its `operation` output is byte-identical to `../../output/kagemusha-100x-20261108/spike/runs/zks-operation-v4.log`. Measured output:
`../../output/kagemusha-100x-20261108/b04a57-commit/runs/operation-v4-pixel-measured.log`.

| P cols / S cols | Send (model → measured) | Receive (model → measured) | Pairs |
|---|---|---|---|
| 16,384 / 16,384 (v4's choice) | 2.37 → 2.84 s | 3.44 → 4.28 s | 5,056 / 6,624 B |
| 32,768 / 16,384 (new fastest) | 2.48 → 2.66 s | 3.55 → 3.87 s | 4,368 / 5,696 B |
| 32,768 / 8,192 | 2.38 → 2.60 s | 3.48 → 3.88 s | 4,544 / 6,112 B |

- The largest variants at 32,768 / 16,384 come to 2.71 / 4.26 s.
- At that shape the two IPAs are 1.24 s of Send and of Receive.
  - That is 47% of Send's time and 32% of Receive's.
  - They are still modelled (0.17 s + 18.3 µs per column), not measured at 16,384 or 32,768.
- Receive's best chunk width moves from 13 to 14 bits. 14 bits is interpolated, not measured.

### Why the model misses: iroha_pasta's MSM planner

From `crates/iroha_pasta/src/msm/pippenger.rs` (`plan`, `model_cost`, `run_task_cancellable`, `Buckets`).
`model_cost = windows × (6n + 25·2^(c−1))`, divided by the achieved concurrency, leaves out three effects:

1. **Chunked tasks repeat the bucket reduction.** With `chunks > 1`, every chunk reduces its own buckets, so the
   reduction is paid `chunks` times. Example: 11-bit rows at 16,384 columns plan one 12-bit window split into 8
   chunks. Each chunk reduces 2,048 buckets for only 2,048 points.
2. **The batch holds a quarter of a task's buckets** (`cap = (count / 4).clamp(16, 1024)`).
   - 13-bit rows plan 7-bit windows: 64 buckets per task, so 16 additions share each inversion.
   - 16-bit rows plan 9-bit windows: 256 buckets, 64 per inversion.
   - That is the 2x between 13 and 16 bits.
   - A top window with few distinct digits also sends its repeats to projective overflow additions.
3. **Waves.** When tasks outnumber `concurrency`, they run in waves, and each wave waits for its slowest task.
   - Full width at 16,384 columns runs 14 tasks in two waves.
   - On the Pixel's 2+2+4 cores, each wave lasts as long as an A55 task.

### Lever: commit a proof's rows in lanes (Mac only so far)

The prover commits its 8-64 rows one after another, each MSM spread over the whole pool. The `commit-lanes`
mode of `../../output/kagemusha-100x-20261108/b04a57-commit/` instead gives each row one single-threaded plan, with lanes claiming the next
row. This is the pattern of `msm_secret_columns_cancellable`, here for bounded widths as well; results are
checked equal to the per-row loop.

On the Mac at 8 threads (shared, load about 24), lanes win at every width:
- full width: 1.1-2.3x;
- narrow rows: 1.5-4x.

Committing 13-bit rows with a 16-bit plan is faster in both modes. The Pixel run (`commit-lanes` plus
`ipa-split`, about 5 minutes) waits for the owner's go.

## Lane commits and the IPA on the Pixel (2026-11-08 11:31Z, owner "Yes, run it")

Binary: `../../b04a57-commit` (source fingerprint b6d99c5a…, binary sha256 353595e2…). iroha_pasta is unchanged
from 7f788ce30f to HEAD 9f36091798. 8 threads.
- `commit-lanes`: 105 s, peak RSS 84 MB, battery 32.8 → 33.0 °C.
- `ipa-split`: 24 s, 51 MB, 33.0 °C.
- Logs: `../../output/kagemusha-100x-20261108/b04a57-commit/runs/{commit-lanes,ipa-split}-pixel6-8t.log`.

### Lane commits

16-row batches, 3 interleaved rounds, medians in µs per entry. Each cell is the per-row loop (each MSM on the
whole pool) → lanes (one single-threaded plan per row).

| columns | 1b | 11b | 13b | 13b, 16-bit plan | 16b | 20b | 88b | 180b | 255b |
|---|---|---|---|---|---|---|---|---|---|
| 8,192 | 0.210 → 0.153 | 0.909 → 0.491 | 0.704 → 0.388 | 0.530 → 0.269 | 0.544 → 0.274 | 0.695 → 0.289 | 2.790 → 0.955 | 3.408 → 1.326 | 4.117 → 2.405 |
| 16,384 | 0.204 → 0.094 | 0.585 → 0.144 | 0.956 → 0.287 | 0.574 → 0.235 | 0.629 → 0.224 | 0.616 → 0.246 | 1.736 → 0.775 | 1.943 → 1.110 | 3.554 → 2.143 |
| 32,768 | 0.205 → 0.080 | 0.400 → 0.113 | 0.655 → 0.288 | 0.455 → 0.222 | 0.481 → 0.199 | 0.454 → 0.184 | 1.420 → 0.743 | 1.632 → 1.006 | 2.636 → 1.978 |
| 65,536 | 0.199 → 0.085 | 0.298 → 0.105 | 0.449 → 0.127 | 0.387 → 0.233 | 0.384 → 0.202 | 0.349 → 0.186 | 1.287 → 0.680 | 1.657 → 0.971 | 2.498 → 1.916 |

- 14-bit rows (own plan or 16-bit plan) are omitted; they match 16-bit within about 10%.
- Full width through iroha_pasta's `msm_secret_columns_cancellable` takes 2.231 / 2.141 / 2.024 / 1.921 µs, the
  same as the lanes column.
- Every lane result equals the per-row loop's.

### The IPA

The spike's `ipa::prove` with full-width challenges; seconds, median of 3.

| n | ipa::prove | model | L/R MSMs | generator fold | rest | fold, 128-bit challenges |
|---|---|---|---|---|---|---|
| 8,192 | 0.283 | 0.32 | 0.107 | 0.148 | 0.021 | 0.139 |
| 16,384 | 0.493 | 0.47 | 0.202 | 0.248 | 0.019 | 0.230 |
| 32,768 | 0.855 | 0.77 | 0.325 | 0.492 | 0.028 | 0.447 |
| 65,536 | 1.473 | 1.37 | 0.504 | 0.910 | 0.032 | 0.845 |

The fold (`iroha_pasta::fold`) splits the challenge with GLV into two halves of about 128 bits, recoded in
width-5 wNAF. A 128-bit challenge still leaves a ~128-bit first half, so it saves only about 7%.

Estimate, not measured: a challenge in endoscalar form, a + b·λ with 64-bit a and b, would hand the fold 64-bit
halves directly. That halves its doublings, roughly −0.25 s per operation at 16,384 columns. The recursive
verifier already assumes 128-bit endoscalar challenges.

### Rescore with every measured term

Run with `b04a57-commit operation`; logs `../../output/kagemusha-100x-20261108/b04a57-commit/runs/operation-v4-pixel-*.log`.

| commits / IPA | fastest shape | Send | Receive | largest variants | pairs |
|---|---|---|---|---|---|
| model / model (v4) | P 16k / S 16k | 2.37 s | 3.44 s | 2.43 / 3.86 s | 5,056 / 6,624 B |
| per-row loop, measured / model | P 32k / S 16k | 2.66 s | 3.87 s | 2.71 / 4.26 s | 4,368 / 5,696 B |
| per-row loop / IPA, both measured | P 32k / S 8k | 2.65 s | 3.93 s | 2.70 / 4.32 s | 4,544 / 6,112 B |
| **lanes / IPA, both measured** | **P 16k / S 16k** | **2.28 s** | **3.24 s** | **2.33 / 3.57 s** | 5,056 / 6,624 B |

Lane costs come from 16-row batches. A proof with fewer rows is costed as split into column pieces; the fastest
shape has none.

Ordinary Receive with lanes, 3.24 s in all:
- P-recv: outer 0.27, inner 0.42, commit 0.90, density 0.05, IPA 0.49 = 2.13 s;
- S-recv: outer 0.07, inner 0.10, commit 0.43, IPA 0.49 = 1.11 s.

The two IPAs are 0.98 s: 30% of Receive and 43% of Send.

**Still to cut for Receive at 3 s:** 0.24 s for an ordinary Receive and 0.57 s for the largest variant. Candidates:
- endoscalar-form IPA challenges (fold about 2x);
- the IPA's two L/R MSMs run concurrently, or with the planner fixed;
- per-wallet key tables plus lookup SHA-256, to bring P-recv toward 2^19.

`model_cost` in iroha_pasta still misses the three effects above. Production PLONK already commits full-width
columns in lanes (iroha_plonk `commit_secret_columns_cancellable`); its bounded commits and the IPA's L/R MSMs
still go through the planner.

## iroha_pasta MSM scheduling and planner, measured on the Pixel (2026-11-08 13:36Z)

Owner: "Yes, do it here", then "Yes, run it". The iroha commits are on optimizations, all signed:
- 069d0ebc3c: claiming loops replace the waves. `msm_secret_bounded_columns_cancellable` adds lane commits at
  each column's own bound.
- f7b4aa7f5c: the planner cost model, fitted to the sweep below.

### Planner sweep

iroha_pasta's ignored `plan_sweep_experiment`: 497 candidate plans over 16 workloads, 3 rounds, 65 s, every result
equal to the planner's own plan. Log: `../../output/kagemusha-100x-20261108/b04a57-commit/runs/plan-sweep-pixel6.log`. Fit:
`../../output/kagemusha-100x-20261108/b04a57-commit/plan_sweep_fit.py`.

| workload (points, bits, workers) | before f7b4aa7f5c | after | fastest measured |
|---|---|---|---|
| 16,384, 255, 8 | 43.1 ms | 43.2 ms | 40.9 ms |
| 32,768, 255, 8 | 83.8 ms | 75.0 ms | 74.0 ms |
| 65,536, 255, 8 | 148.1 ms | 140.5 ms | 138.7 ms |
| 8,192, 255, 8 | 26.1 ms | 22.9 ms | 22.6 ms |
| 16,384, 88, 8 | 24.4 ms | 18.9 ms | 17.8 ms |
| 16,384, 13, 8 | 16.2 ms | 13.9 ms | 12.2 ms |
| 16,384, 11, 8 | 21.1 ms | not measured (c = 7, 2 windows, 8 chunks) | 10.1 ms |
| 16,384, 13, 1 | 14.2 ms | 10.8 ms | 10.7 ms |

- The geometric mean of picks against the fastest measured plan is 1.07 after, against 1.19 before. The worst
  measured slowdown is 2.2%.
- Slow cores dominate what is left. The same 11-bit plan takes 11.9 ms as 4 tasks and 21.1 ms as 8, and full width
  runs fastest as 22-26 single-window tasks. A planner that knows the big and little core counts is the next lever.

### Benchmarks against commit 1, compared with the morning's waves

- `commit-classes`, µs per entry:
  - full width at 16,384 columns: 3.76 → 2.80;
  - full width at 8,192: 4.47 → 3.27;
  - 88-bit at 16,384: 1.82 → 1.17;
  - 64-bit at 16,384: 2.24 → 1.04;
  - unchanged at 32,768 and 65,536, where the old plan already ran a single wave.
- `commit-lanes`: lanes are unchanged (2.16 µs at full width, 16,384 columns). The per-row loop dropped from 3.55
  to 2.84.
- `ipa-split` ran at 34.3 °C after about 5 minutes of load. The unchanged fold slowed by 16-18%, so this is thermal.
  - Endoscalar challenges (u = aζ + b, 65-bit halves) give no fold speedup: 1.046 s against 1.055 s at 65,536.
  - Cause, checked on the Mac: `curve/endo.rs::decompose` truncates its Babai coefficients (floor), so all 1,000
    endoscalar challenges come back with 127-128-bit halves (`glv-check` mode). Rounding to nearest would hand the
    fold 65-bit halves and roughly halve it. Not changed; it waits for the owner.

## IPA with the GLV rounding fix (2026-11-08 14:35Z, owner "Yes, run it")

iroha 8ca1643b3f rounds the GLV coefficients to nearest, so endoscalar challenges u = aζ + b keep their 65-bit
halves. Two `ipa-split` runs at 32.5 °C, 3 rounds each. Each IPA was run with full-width challenges and with
endoscalar challenges; values are the mean of the two runs' medians.

| length | morning table | full-width challenges | endoscalar challenges |
|---|---|---|---|
| 8,192 | 0.283 s | 0.282 s | 0.216 s |
| 16,384 | 0.493 s | 0.435 s | 0.332 s |
| 32,768 | 0.855 s | 0.736 s | 0.525 s |
| 65,536 | 1.473 s | 1.310 s | 0.929 s |

- The fold is 1.77-1.85x faster with endoscalar challenges (16,384: 0.248 → 0.140 s).
- The L/R MSMs are 15-23% faster than in the morning (16,384: 0.202 → 0.164 s), from claiming loops and the
  planner.
- Values are sums of L/R MSMs, fold and rest; with full-width challenges they match `ipa::prove` within about 3%.
  They exclude the spike's transcript hashing.
- Logs: `../../output/kagemusha-100x-20261108/b04a57-commit/runs/ipa-split-pixel6-glvround-run{1,2}.log`.
- At Path 2's shape (two IPAs at 16,384), this is about 0.32 s less per operation than the morning table.

## Fast cores only (2026-11-08 14:49Z, owner "Yes, measure it")

Arms over the parallel sweep workloads: the 8-thread pool, every candidate also with its loops capped at 4 and 2
(143 s); and the process pinned to the four fast cores with `taskset f0` and 4 threads (79 s, 33.3 → 34.4 °C).
Logs: `../../output/kagemusha-100x-20261108/b04a57-commit/runs/plan-sweep-pixel6-{loopcaps,fastcores}.log`.

| workload | 8 loops | capped at 4 or 2 | pinned to fast cores |
|---|---|---|---|
| 16-bit, 16,384 points | 12.5 ms | 7.2 ms | 6.5 ms |
| 13-bit, 16,384 | 10.7 ms | 8.1 ms | 7.0 ms |
| 11-bit, 16,384 | 13.3 ms | 9.2 ms | 8.1 ms |
| 13-bit, 32,768 | 17.9 ms | 10.4 ms | 11.8 ms |
| full width, 16,384 | 40.7 ms | 47.6 ms | 52.4 ms |
| full width, 65,536 | 139.4 ms | 165.6 ms | 173.9 ms |

- Narrow widths gain from fewer loops, but the winners are erratic. The same narrow plan ran 2x faster on 2 loops
  than on 4, even when pinned. These 5-20 ms kernels are dominated by OS placement and frequency ramp-up.
- Full width and wide bounded widths stay fastest on all 8 cores.
- No planner change. Narrow columns should go through lane commits, which balance across unequal cores. Path 2
  already does this.
