# Real Receive circuit, stage A (2026-11-09)

Historical research record. See [README.md](README.md) for the current scope, reproduction commands and unresolved production gaps.

Owner request: "Real Receive circuit".

- Stage A builds the Receive pair at full size from real constraints and proves it.
- Stage B adds the exact KAGEMUSHA field layouts and encodings.

Run it with `ZKS_SCHEDULE=lean zk-spartan real-receive`.

- `ZKS_REPEAT=N` proves each circuit N times. Each proof prints its core: commits, helpers, sumchecks and IPA, with
  the matrix build timed inside the same call and listed apart. With N > 1 it also prints medians.
- `ZKS_REFUSE=1` adds the full-size refusal checks.
- Memory is printed at every stage: peak and current RSS on Android and Linux; resident size and physical footprint
  on macOS.

## What is proved

**P-recv**: over Fp, committed on Vesta. It contains:

- The own Receive relation: comb16 own-key ECDSA and lookup SHA-256, 106,955 rows.
- The synthesized verifiers of the two incoming pairs, taken from real proofs:
  - the pairs are the wallet's previous Receive pair and the payer's Send pair;
  - P-recv holds the scalar side of each P proof and the point side of each S proof.
- The non-native binding chain over the 258 shared values.
- 29,710 filler constraints, each with a fresh full-width cell. They stand for constraints the census counts but
  the spike does not synthesize:
  - transcript permutations;
  - the 8,000-constraint matrix-MLE budget per verified proof.

**S-recv**: over Fq, committed on Pallas. It mirrors P-recv:

- the point side of each P proof and the scalar side of each S proof;
- the native binding chain;
- the same filler.

**The prover** (`src/sparse.rs`) proves general sparse R1CS with a real LogUp argument:

- Phase 1 commits the witness and the table multiplicities.
- α and β are drawn from the transcript after the phase-1 rows.
- Phase 2 commits the helpers h = 1/(α − f) and g = m/(α − t), computed by batch inversion.
- One row checks Σh = Σg. Tuples are compressed as (tid + 1) + Σ β^j f_j.
- The witness is laid out by width class. Each row commits at the width of its widest entry.

## Results

Mac, 16 threads. Log: `runs/zks-real-receive-v3.log`; refusals in `runs/zks-real-receive-v4-refuse.log`. The Mac is
shared, so S-recv varied between runs.

| | P-recv | S-recv |
|---|---:|---:|
| Constraints | 2^19 (368,601 used; census 363,768) | 2^18 (195,283 used) |
| Lookups / table entries | 71,200 / 5,120 | 0 / 0 |
| Witness | 366,242 + 76,320 helpers, 32 rows × 16,384 | 195,651, 32 rows × 8,192 |
| Rows by width (bits: rows) | 1: 4, 3: 1, 11: 4, 177: 1, 255: 22 | 1: 7, 255: 25 |
| Nonzeros in A, B, C | 5.74 M (15.6 per constraint) | 2.56 M (13.1 per constraint) |
| Proof | 2,464 B | 2,400 B |
| Mac prove, per proof | 0.31 s (commit 0.16, outer 0.03, inner 0.04, open 0.09) | 0.17-0.26 s |
| Matrix build (one-time preprocessing) | 0.44 s; 0.11-0.17 s with parallel sorts (v5) | 0.19 s; 0.05-0.07 s (v5) |
| Mac verify | 0.47 s, of which 0.44 s is the matrix build | 0.21 s, of which 0.19 s |
| **Pixel model, this exact layout** | **1.56 s** | **0.88 s** |
| Pixel model, census (gate 1f) | 1.47 s | 0.84 s |

The Pixel model for the real layout uses the same Pixel-measured costs as the census model: lane commits, and the
IPA after the GLV fix. They are applied to this circuit's row widths and nonzero count.

- **Receive: 2.44 s** modelled on the Pixel, against 2.31 s from the census.
- The gap is density, +0.16 s. Fewer full-width entries in P's rows offset 0.03 s of it.

**Refusals.** In each proof, one constrained full-width cell was set off by one: early, in the middle, and at the
binding chain's end. All six proofs were rejected. Filler stand-ins and padding cells are unconstrained by design.

## Pixel 6 run (2026-11-09 00:03Z)

Owner: "Yes, run on Pixel". The phone operator, b04a57, ran it after the owner's go in its own chat.

- Inputs: binary sha256 92b291dd… (source fingerprint 9f35f4b4…) and `run.sh` 10e1e5d2…; the operator checked both
  hashes on the device.
- Two runs of `ZKS_REPEAT=3` on 8 threads, battery at 29.6-30.6 °C. Both exited 0 after 31 s, and all 12 proofs
  were accepted.
- Logs: `../../output/kagemusha-100x-20261108/spike/runs/zks-real-receive-pixel6-run{1,2}.log`.

Median of three proofs per run (run 1 / run 2), compared with the Pixel model for this layout:

| seconds | P-recv | P model | S-recv | S model |
|---|---:|---:|---:|---:|
| Commit, phase 1 | 0.713 / 0.745 | ≈ 0.57 | 0.545 / 0.561 | 0.43 |
| Commit, phase 2 (LogUp helpers) | 0.274 / 0.292 | ≈ 0.16 | 0.004 / 0.003 | 0 |
| Helper inversion | 0.022 / 0.018 | not modelled | 0 | 0 |
| Outer + inner sumchecks | 0.523 / 0.530 | 0.49 (incl. density 0.14) | 0.298 / 0.286 | 0.23 (incl. density 0.06) |
| Open (`relation()` + IPA) | 0.579 / 0.595 | IPA 0.34 | 0.342 / 0.360 | IPA 0.23 |
| **Core** | **2.105 / 2.157** | **1.56** | **1.176 / 1.223** | **0.88** |

(The model's phase split follows its entry counts: the ~76,000 helpers are about 0.16 s of P's 0.73 s commit.)

**Receive pair: 3.28 / 3.38 s, 1.34-1.39x the 2.44 s model.** That is about 90x faster than today's ~292 s folded
Receive, counting the prover core only.

The gap is 0.89 s per pair:

1. **Commits, +0.40 s. Only about 0.1 s of it is the lane tail.** (Corrected after b04a57's Pixel A/B of the fix.)
   - Each row commits as one single-threaded plan, and lanes claim whole rows. A commit call ends when its last row
     finishes, and on the Pixel's 2+2+4 cores that is often a slow core.
   - Phase 2 shows the tail directly. Its helpers fill five rows, which run side by side, so the phase lasts one
     single-threaded row: 0.27-0.29 s, against 0.16 s at the per-entry rate. The window-claim fix (iroha 28ef3cd191)
     brings the same call from 0.240 to 0.176 s in b04a57's Pixel harness.
   - Phase 1 is not mainly a tail. The harness runs the same calls with the old lanes at P 0.62 s and S 0.46 s, only
     7-8% over the per-entry rate (0.57 / 0.43), and the fix saves 2-4% there. But the real run measured
     0.71-0.75 s (P) and 0.55 s (S) for the same calls. That leaves about 0.1 s per proof unexplained.
   - It is not repeated values in the real rows. On the Mac, the real phase-1 rows commit as fast as random values at
     the same positions and bounds: P 0.091 vs 0.093 s, S 0.087 vs 0.093 s (`../../output/kagemusha-100x-20261108/spike/runs/zks-real-receive-v7-rowstats.log`).
   - Resolved by b04a57's Pixel runs (logs: `../../output/kagemusha-100x-20261108/spike/runs/zks-real-receive-ab-pixel6-*`, `…-rowstats-pixel6-*`,
     `../../output/kagemusha-100x-20261108/commit-tail/runs/same-conditions/`). It is not the kernel:
     - The bound checks and blinding take about 15 ms per P proof inside the spike's timer.
     - Run side by side under the same conditions (33.6 °C), the spike's commit MSM is only 3% (P) and 7% (S)
       slower than the harness. S commits on Pallas, while the harness used Vesta.
     - Thread samples show the same core split in both processes.
     - The rest of the earlier 15-20% was measurement conditions. Each commit follows single-threaded phases of the
       proving flow (matrix build, the previous verify), and phone state moves timings up to ~10% run to run.
     - Ruled out on the way: data (real rows are no slower than random ones), compiler settings, page faults,
       process size and row shapes.
   - The uniform layout changes this flow anyway: it has no matrix build.
2. **Sparse matrix evaluation inside the opening, about +0.37 s.**
   - "Open" includes `relation()`. It evaluates the A, B and C MLEs at (r_x, r_y) over all 5.74 M (P) or 2.56 M (S)
     nonzeros, after eq tables of 2^19 + 2^20 entries (P). The model counts only the IPA.
   - Open minus the Pixel IPA table is 0.24-0.25 s (P) and 0.12-0.13 s (S). This also includes the row combination
     and a 72-point MSM, both small. This share is estimated by subtraction, not timed on its own.
   - Verification consistently takes 0.28-0.32 s (P) and 0.16-0.18 s (S); it does the same evaluation plus one
     generator MSM.
   - The uniform layout evaluates the matrices from block structure, so this term goes away for the prover and both
     verifiers.
3. **Sumchecks, +0.10 s.** P is within 7% of outer + inner + density. S is 25-30% over (+0.06 s).
4. **Helper inversion, +0.02 s.** Not modelled.

The window-claim fix recovers about 0.1 s, and the uniform layout removes item 2 (about 0.37 s). That gives about
2.9 s per Receive pair. If the unexplained ~0.2 s per pair of phase-1 commit time is recovered too, it comes to about
2.7 s. The uniform layout adds the copy lookups that the census charges, so its own cost still has to be measured
(stage A2).

**Outside the core:**

- Matrix build per proof: 0.72-0.74 s (P) and 0.33-0.34 s (S); on the Mac, 0.11-0.17 s and 0.05-0.07 s. The spike
  rebuilds the matrices for every proof because α and β enter the LogUp rows. The uniform layout has no per-proof
  build.
- Verification: 0.28-0.32 s plus 0.72-0.75 s of matrices (P); 0.16-0.18 s plus 0.33-0.37 s (S).
- Assembly: 11.1-11.6 s. This covers Gens derivation, proving the two synthetic incoming pairs, and synthesizing
  their verifiers and the own relation. Harness work and per-Receive witness work are mixed, so this is not the
  per-Receive witness time.

**Memory** (VmHWM, from both `run.sh` and the binary):

- The peak was 1.10 GB (1.03 GiB) in both runs, and it stayed flat across all six proofs of each circuit. The
  growing resident size on the Mac was the allocator, as diagnosed.
- By stage:
  - 178 MB after the own relation;
  - 612 MB with both incoming pairs;
  - 519 MB once assembled;
  - +550 MB for the P-recv matrix set (1,088 MB at the build);
  - 539 MB between P proofs and 241 MB during S.
- The production prover's admission is 0.68 GiB counted peak, and stage A peaks at 1.5x that. Both the matrix set
  and the gadget-level assembly are stage-A structures that the uniform layout replaces.

## Fixes the real circuit forced

- **Negative evaluation products.**
  - The problem: the non-native multiplication checks evaluate a(t)·b(t) at t = −1, where the product can be
    negative. Its field element is full width, but the declared width came from |product|, so the commit's bound
    check failed.
  - The fix: offset encoding, storing product + 2^bits one bit wider.
  - The layout also moves any value wider than its declared width to full width. None remain.
- **Filler cells.** Filler rows first reused existing cells. That undercounted the witness by 29,710 full-width
  entries per proof, about 0.06 s per proof on the Pixel model. Each filler row now gets a fresh cell.
- **Helpers by batch inversion.** The 76,320 inversions now cost under 0.01 s.

## What stage A does not show yet

1. **The uniform layout. This is the next structural step.**
   - Stage A wires the circuit directly, as an arbitrary sparse matrix with 5.74 M nonzeros. The next proof's
     verifier cannot evaluate that matrix's MLE inside its 8,000-constraint budget.
   - REPORT4 and the b04a57 soundness review already require a uniform-block compiler: about ten block types,
     with wiring through copy lookups into a value table.
   - The census charges those copy lookups. Stage A does not build them.
   - So stage A validates two things on a real constraint system: the gadget counts (within 1.3%) and the
     prover's cost structure. It does not measure the uniform layout's own cost.
2. **The circuit description in memory.**
   - Naive CSR + CSC with 32-byte coefficients takes about 400 MB for P-recv.
   - Building it took 0.44 s on the Mac with single-threaded sorts, and takes 0.11-0.17 s with parallel sorts.
   - Measured physical footprint on the Mac (`runs/zks-real-receive-v6-memory.log`):
     - 176 MB after the own relation;
     - 630 MB once both incoming pairs are synthesized and assembled;
     - 1.25 GB with one P-recv matrix set alive.
   - The resident size grows by one freed matrix set per proof: 82 B per nonzero in both proofs, up to 3.9 GB after
     five P-recv proofs. This is not a leak. macOS keeps freed large blocks until the kernel reclaims them. Nothing
     in the spike retains them, and safe Rust has no way to hold them here.
   - The Pixel confirms this: VmHWM peaks at 1.10 GB (1.03 GiB) and stays flat across proofs.
   - The production wallet prover's admission is 0.68 GiB counted peak, set for 4 GB phones. Stage A peaks at 1.5x
     that on the Pixel.
   - A phone prover needs the description preprocessed and compact. The uniform layout gives that: block types
     plus index columns.
3. **Work outside the prover core.** Witness generation, native checks and durable completion are still not
   measured.

The Pixel run has now measured the density cost and the phase-2 commit; see above.

## Next

- **Commit tails** (owner: "Commit-tail fix"), about 0.4 s per Receive pair. b04a57 is writing it as the only
  writer for `iroha_pasta/src/msm`, because it wrote the lanes API.
  - `secret_columns` now claims window ranges of columns, widest first. Each claim takes about 1/(4 · loops) of the
    windows left, so the batch ends on single windows. No window is reduced twice, and results are exact.
  - Reviewed from source: diff `../../output/kagemusha-100x-20261108/commit-tail/iroha_pasta-window-claims.diff`, sha256 410945045d60….
  - Committed as iroha 28ef3cd191 (signed). The code is that diff; the commit adds the doc wording and the
    cross-thread cancellation test from the review. iroha_pasta lib tests: 139 pass; iroha_plonk lib tests: 462
    pass.
  - Mac A/B on this spike, built against exactly that diff: 3 interleaved rounds, load 28-37, all 36 proofs
    accepted. Logs in `../../output/kagemusha-100x-20261108/lane-tails/runs/ab-*.log`.

    | Mac, seconds | Old | New |
    |---|---:|---:|
    | P commit | 0.166 | 0.112 |
    | P phase 1 | 0.109 | 0.086 |
    | P phase 2 | 0.056 | 0.027 |
    | S commit | 0.086 | 0.065 |
    | P core | 0.338 | 0.286 |
    | S core | 0.199 | 0.167 |

  - The Pixel A/B (old against claim sizes of 1/4 and 1/8) waits for the owner's go in b04a57's chat.
- **Stage A2**, started 2026-11-09; status and plan in `A2-UNIFORM.md`. The block census shows that no grouping of
  today's gadgets is uniform within budget, so the gadgets become uniform steps with structured wiring. It also
  removes:
  - the sparse matrix evaluation (about 0.37 s per pair);
  - the per-proof matrix build;
  - the matrix set's ~550 MB.
- **Stage B**: the exact KAGEMUSHA field layouts and encodings.
- The next phone binary should time `relation()` separately from the IPA. Gap item 2 is then measured instead of
  estimated by subtraction.
- The Pixel run's files stay in `../../output/kagemusha-100x-20261108/spike/pixel-real-receive/`:
  - `zk-spartan`: sha256 92b291dd…; source fingerprint 9f35f4b4… (sha256 of
    `shasum -a 256 Cargo.toml Cargo.lock src/*.rs`); iroha 8abae71ea2; NDK 28.0.12674087, API 26.
  - `run.sh`: sha256 10e1e5d2….

## Fixed-base table commits wired in (2026-11-09)

Owner: "Yes, wire tables". This uses b04a57's batched table path (iroha 2b50a7f647,
`FixedBaseTable::msm_secret_bounded_columns_cancellable`). On the Pixel it measured about −0.30 s per Receive
pair: P first phase 635 → 478 ms, helper rows 200 → 162 ms, S 468 → 353 ms.

**How it is wired:**

- `Gens` caches one table per (active columns, window), built on first use over exactly `g[..active]`.
- Windows: c14 for P (Vesta) and c13 for S (Pallas). These are b04a57's Pixel choices;
  `ZKS_TABLE_WINDOW_P` and `ZKS_TABLE_WINDOW_S` override them.
- `commit_rows_lanes` calls the table in place of the variable-base column MSM. Blinds and affine conversion are
  unchanged, and the full-width fallback for out-of-class rows still applies.
- `ZKS_COMMIT=variable` keeps the old path, so a phone A/B differs only in the commit path.
- `real-receive` builds the table before the timed proofs and prints its build time.

**Also new:** every proof line splits `open` into `relation` (the batched relation, including the matrix
evaluation) and the rest. That measures this note's gap item 2 directly instead of by subtraction.

**Checks:**

- `table_commits_equal_variable_base`: on both curves, rows of width 255, 88, 32, 11 and 1, plus an all-zero row,
  give the same group elements by both paths.
- All 24 spike tests pass with tables as the default.

**Mac A/B** (`runs/zks-real-receive-v9-{variable,table}-run{1,2}.log`, interleaved). The machine was heavily
loaded (load average ~57), so treat absolute times with care.

- P-recv commit: 0.25-0.36 s with tables, against 0.38-0.44 s variable-base. Every table proof was below every
  variable-base proof.
- S-recv: inconclusive here.
- Table build: P 0.17-0.28 s, S 0.09-0.10 s.
- All 24 proofs were accepted.
- `relation` on the Mac: P 0.09-0.20 s, S 0.05-0.19 s (noisy).

**Phone A/B package** (`../../output/kagemusha-100x-20261108/spike/pixel-real-receive-tables/`):

- binary `zk-spartan-tables`, sha256 230eb584…;
- `run-ab.sh`, sha256 e1ec9c3e…, which interleaves variable-base and table runs twice with `ZKS_REPEAT=3`;
- a README with the source fingerprint.

The run waits for the owner's go in b04a57's chat.

### Pixel A/B result (2026-11-09 10:58-11:02Z, b04a57, after the owner's go)

The run order was variable, table, table, variable (`commit-tail/run-real-receive-tables.sh`, sha256 9c746b91…, the
same binary 230eb584…). The phone was at 22.4-24.0 °C with MemAvailable 2.94-3.04 GB. Logs:
`../../output/kagemusha-100x-20261108/spike/runs/zks-real-receive-tables-pixel6-{variable,table}-run{1,2}.log`. All 24 proofs were accepted. The means
below are of 6 proofs per path, re-parsed from the logs:

| | variable-base | fixed-base table |
|---|---:|---:|
| P-recv commit | 0.789 s (phase 1 0.590, phase 2 0.199) | 0.621 s (0.463, 0.158): 0.79x |
| S-recv commit | 0.449 s (0.446, 0.003) | 0.349 s (0.336, 0.013): 0.78x |
| commits per pair | 1.238 s | 0.970 s (−0.268 s) |
| core per pair | 2.812 s | 2.566 s |
| the rest of each proof (untouched code) | P 1.014, S 0.559 s | P 1.012, S 0.584 s (noise about 0.03 s) |

- **The saving is about 0.27 s per Receive pair**, all of it in the commits.
- **The phone was cool.** This morning's variable-base pair core was 3.26-3.30 s at 29-33 °C, against 2.81 s
  here. Combine with older runs by ratio: this morning's 3.28-3.38 s becomes about 3.0-3.1 s with tables at
  those temperatures.
- **Gap item 2, measured.** `relation` inside open (the batched relation, including the sparse matrix
  evaluation) is P 0.178 s (0.152-0.202) and S 0.091-0.104 s (0.077-0.128). That is about 0.27 s per pair, a third
  of open. The earlier estimate by subtraction was 0.37 s. The uniform layout's compiled evaluation plan targets
  this.
- **Table builds** are one-time per process: P c14 0.41-0.43 s, S c13 0.22-0.27 s. They land on the first payment
  after a cold start unless the app builds them earlier (at unlock, say). VmHWM went 1.082 → 1.100 GB.
- **Remaining inefficiency.** S phase 2's +10 ms comes from its five empty rows: every table claim allocates and
  reduces the full 2^(c−1) bucket set, even for a 1-bit column. Sizing buckets from the public bound when a column
  has one window should remove it, and likely trims P's empty and narrow rows too. b04a57 is proposing that
  iroha_pasta change to the owner.
